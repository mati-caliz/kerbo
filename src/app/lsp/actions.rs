use serde_json::{Map, Value, json};

use super::{diagnose, path_from_uri, to_lsp, utf16_column};
use crate::fix::{TextEdit, TextPoint, compatible, edits_of};
use crate::i18n::{Lang, Text};
use crate::rules::Diagnostic;

pub const QUICK_FIX: &str = "quickfix";
pub const FIX_ALL: &str = "source.fixAll";
const FIX_ALL_KERBO: &str = "source.fixAll.kerbo";
const KIND_SEPARATOR: char = '.';
const SOURCE: &str = "kerbo";

struct Request<'request> {
    first: usize,
    last: usize,
    only: Option<Vec<&'request str>>,
    reported: Vec<&'request Value>,
}

impl<'request> Request<'request> {
    fn from_params(params: Option<&'request Value>) -> Self {
        let line_at = |pointer: &str| {
            params
                .and_then(|params| params.pointer(pointer))
                .and_then(Value::as_u64)
                .and_then(|line| usize::try_from(line).ok())
        };
        let only = params
            .and_then(|params| params.pointer("/context/only"))
            .and_then(Value::as_array)
            .map(|kinds| kinds.iter().filter_map(Value::as_str).collect());
        let reported = params
            .and_then(|params| params.pointer("/context/diagnostics"))
            .and_then(Value::as_array)
            .map(|diagnostics| {
                diagnostics
                    .iter()
                    .filter(|diagnostic| is_ours(diagnostic))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            first: line_at("/range/start/line").unwrap_or_default(),
            last: line_at("/range/end/line").unwrap_or(usize::MAX),
            only,
            reported,
        }
    }

    fn wants(&self, kind: &str) -> bool {
        self.only.as_ref().is_none_or(|only| {
            only.iter().any(|asked| {
                kind == *asked
                    || kind
                        .strip_prefix(asked)
                        .is_some_and(|rest| rest.starts_with(KIND_SEPARATOR))
            })
        })
    }

    fn asked_for(&self, diagnostic: &Diagnostic) -> bool {
        let line = diagnostic.line.saturating_sub(1);
        if self.reported.is_empty() {
            return (self.first..=self.last).contains(&line);
        }
        self.reported.iter().any(|reported| {
            reported.get("code").and_then(Value::as_str) == Some(diagnostic.rule.as_str())
                && reported
                    .pointer("/range/start/line")
                    .and_then(Value::as_u64)
                    .and_then(|start| usize::try_from(start).ok())
                    == Some(line)
        })
    }
}

fn is_ours(diagnostic: &Value) -> bool {
    diagnostic
        .get("source")
        .and_then(Value::as_str)
        .is_none_or(|source| source == SOURCE)
}

pub fn code_actions(uri: &str, text: &str, params: Option<&Value>, lang: Lang) -> Value {
    let Some(path) = path_from_uri(uri) else {
        return Value::Array(Vec::new());
    };
    let request = Request::from_params(params);
    let lines: Vec<&str> = text.lines().collect();
    let diagnostics = diagnose(&path, text);
    let mut actions = Vec::new();
    if request.wants(QUICK_FIX) {
        actions.extend(
            diagnostics
                .iter()
                .filter(|diagnostic| request.asked_for(diagnostic))
                .filter_map(|diagnostic| quick_fix(uri, diagnostic, &lines, lang)),
        );
    }
    if request.wants(FIX_ALL_KERBO) {
        actions.extend(fix_all(uri, &diagnostics, &lines, lang));
    }
    Value::Array(actions)
}

fn quick_fix(uri: &str, diagnostic: &Diagnostic, lines: &[&str], lang: Lang) -> Option<Value> {
    let edits = &diagnostic.fix.as_ref()?.edits;
    Some(json!({
        "title": format!("{SOURCE}: {}", lang.explain(diagnostic.rule).fix),
        "kind": QUICK_FIX,
        "isPreferred": true,
        "diagnostics": [to_lsp(diagnostic, lines, lang)],
        "edit": workspace_edit(uri, edits, lines),
    }))
}

fn fix_all(uri: &str, diagnostics: &[Diagnostic], lines: &[&str], lang: Lang) -> Option<Value> {
    let chosen = compatible(
        diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.fix.as_ref()),
    );
    if chosen.is_empty() {
        return None;
    }
    Some(json!({
        "title": format!("{SOURCE}: {}", lang.text(Text::FixAllTitle)),
        "kind": FIX_ALL_KERBO,
        "edit": workspace_edit(uri, &edits_of(&chosen), lines),
    }))
}

fn workspace_edit(uri: &str, edits: &[TextEdit], lines: &[&str]) -> Value {
    let edits: Vec<Value> = edits
        .iter()
        .map(|edit| {
            json!({
                "range": { "start": position(edit.start, lines), "end": position(edit.end, lines) },
                "newText": edit.replacement,
            })
        })
        .collect();
    let mut changes = Map::new();
    changes.insert(uri.to_owned(), Value::Array(edits));
    json!({ "changes": changes })
}

fn position(point: TextPoint, lines: &[&str]) -> Value {
    let line = point.line.saturating_sub(1);
    let text = lines.get(line).copied().unwrap_or_default();
    json!({ "line": line, "character": utf16_column(text, point.column.saturating_sub(1)) })
}
