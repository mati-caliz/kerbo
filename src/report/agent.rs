use serde_json::{Value, json};

use crate::i18n::Lang;
use crate::rules::{Diagnostic, RuleId};

const SHOWN_LIMIT: usize = 30;
const CHEATING_RULES: [RuleId; 4] = [
    RuleId::SuppressionAdded,
    RuleId::TestDisabled,
    RuleId::TestRemoved,
    RuleId::TypeEscape,
];

pub(super) fn document(diagnostics: &[Diagnostic], lang: Lang) -> Value {
    let mut ordered: Vec<&Diagnostic> = diagnostics.iter().collect();
    ordered.sort_by_key(|diagnostic| std::cmp::Reverse(priority(diagnostic)));
    let shown: Vec<Value> = ordered
        .iter()
        .take(SHOWN_LIMIT)
        .map(|diagnostic| item(diagnostic, lang))
        .collect();
    json!({
        "problems": diagnostics.len(),
        "shown": shown.len(),
        "items": shown,
        "more": diagnostics.len().saturating_sub(SHOWN_LIMIT),
        "explain": "kerbo explain <rule>",
    })
}

fn priority(diagnostic: &Diagnostic) -> (bool, usize) {
    let cheating = CHEATING_RULES.contains(&diagnostic.rule);
    let excess = diagnostic.value.saturating_sub(diagnostic.limit);
    (cheating, excess)
}

fn item(diagnostic: &Diagnostic, lang: Lang) -> Value {
    json!({
        "id": diagnostic.fingerprint,
        "file": diagnostic.relative_path,
        "line": diagnostic.line,
        "end": diagnostic.end_line,
        "rule": diagnostic.rule.as_str(),
        "value": diagnostic.value,
        "max": diagnostic.limit,
        "fix": lang.explain(diagnostic.rule).fix,
    })
}
