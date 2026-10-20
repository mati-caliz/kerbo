use serde_json::{Map, Value, json};

use super::{diagnose, path_from_uri, to_lsp, utf16_column};
use crate::fix::TextPoint;
use crate::i18n::Lang;
use crate::rules::Diagnostic;

pub const QUICK_FIX: &str = "quickfix";
const SOURCE: &str = "kerbo";

pub fn code_actions(uri: &str, text: &str, range: Option<&Value>, lang: Lang) -> Value {
    let Some(path) = path_from_uri(uri) else {
        return Value::Array(Vec::new());
    };
    let line_at = |pointer: &str| {
        range
            .and_then(|range| range.pointer(pointer))
            .and_then(Value::as_u64)
            .and_then(|line| usize::try_from(line).ok())
    };
    let first = line_at("/start/line").unwrap_or_default();
    let last = line_at("/end/line").unwrap_or(usize::MAX);
    let lines: Vec<&str> = text.lines().collect();
    let actions = diagnose(&path, text)
        .iter()
        .filter(|diagnostic| (first..=last).contains(&diagnostic.line.saturating_sub(1)))
        .filter_map(|diagnostic| action(uri, diagnostic, &lines, lang))
        .collect();
    Value::Array(actions)
}

fn action(uri: &str, diagnostic: &Diagnostic, lines: &[&str], lang: Lang) -> Option<Value> {
    let edits: Vec<Value> = diagnostic
        .fix
        .as_ref()?
        .edits
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
    Some(json!({
        "title": format!("{SOURCE}: {}", lang.explain(diagnostic.rule).fix),
        "kind": QUICK_FIX,
        "isPreferred": true,
        "diagnostics": [to_lsp(diagnostic, lines, lang)],
        "edit": { "changes": changes },
    }))
}

fn position(point: TextPoint, lines: &[&str]) -> Value {
    let line = point.line.saturating_sub(1);
    let text = lines.get(line).copied().unwrap_or_default();
    json!({ "line": line, "character": utf16_column(text, point.column.saturating_sub(1)) })
}
