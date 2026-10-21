use serde_json::{Map, Value, json};

use std::path::Path;

use super::super::fixing::{Fixer, converge};
use super::{diagnose, path_from_uri, to_lsp, utf16_column};
use crate::fix::{TextEdit, TextPoint};
use crate::i18n::{Lang, Text};
use crate::language::Language;
use crate::rules::Diagnostic;

pub const QUICK_FIX: &str = "quickfix";
pub const FIX_ALL: &str = "source.fixAll";
const FIX_ALL_KERBO: &str = "source.fixAll.kerbo";
const KIND_SEPARATOR: char = '.';
const SOURCE: &str = "kerbo";
const LINE_FEED: char = '\n';

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
        let document = Document {
            uri,
            path: &path,
            text,
        };
        actions.extend(fix_all(&document, diagnostics, lang));
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

struct Document<'document> {
    uri: &'document str,
    path: &'document Path,
    text: &'document str,
}

// Las mismas vueltas que `kerbo fix`, con el texto del editor: como el protocolo pide ediciones
// sobre el texto original, se manda una sola que reemplaza el documento entero por el arreglado.
fn fix_all(document: &Document<'_>, diagnostics: Vec<Diagnostic>, lang: Lang) -> Option<Value> {
    let analyze = |text: &str| Some(diagnose(document.path, text));
    let accepts = |diagnostic: &Diagnostic| diagnostic.fix.is_some();
    let fixer = Fixer {
        language: Language::from_path(document.path),
        analyze: &analyze,
        accepts: &accepts,
    };
    let converged = converge(&fixer, document.text, diagnostics)?;
    let replacement = json!({
        "range": { "start": { "line": 0, "character": 0 }, "end": document_end(document.text) },
        "newText": converged.text,
    });
    let mut changes = Map::new();
    changes.insert(document.uri.to_owned(), Value::Array(vec![replacement]));
    Some(json!({
        "title": format!("{SOURCE}: {}", lang.text(Text::FixAllTitle)),
        "kind": FIX_ALL_KERBO,
        "edit": { "changes": changes },
    }))
}

fn document_end(text: &str) -> Value {
    let last_line = text.split(LINE_FEED).count().saturating_sub(1);
    let last = text.rsplit(LINE_FEED).next().unwrap_or_default();
    json!({ "line": last_line, "character": utf16_column(last, last.len()) })
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
