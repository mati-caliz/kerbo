use serde_json::{Map, Value, json};

use super::{AUTOFIX_COMMAND, EXPLAIN_HINT, FileGroup, Report};

const PATCH_KEY: &str = "patch";

pub(in crate::report) fn document(report: &Report<'_>) -> Value {
    let fixes: Map<String, Value> = report
        .fixes
        .iter()
        .map(|fix| (fix.rule.as_str().to_owned(), Value::from(fix.text)))
        .collect();
    json!({
        "problems": report.problems,
        "shown": report.shown(),
        "more": report.more(),
        "files": report.files.iter().map(file).collect::<Vec<Value>>(),
        "fix": fixes,
        "explain": EXPLAIN_HINT,
        "autofix": { "problems": report.fixable, "command": AUTOFIX_COMMAND },
    })
}

fn file(group: &FileGroup<'_>) -> Value {
    let items: Vec<Value> = group
        .rows
        .iter()
        .map(|row| {
            let diagnostic = row.diagnostic;
            let mut item = json!({
                "id": row.id,
                "line": diagnostic.line,
                "end": diagnostic.end_line,
                "rule": diagnostic.rule.as_str(),
                "value": diagnostic.value,
                "max": diagnostic.limit,
            });
            if let (Some(fix), Some(fields)) = (&diagnostic.fix, item.as_object_mut()) {
                fields.insert(PATCH_KEY.to_owned(), Value::from(fix.hunk.as_str()));
            }
            item
        })
        .collect();
    json!({ "path": group.path, "items": items })
}
