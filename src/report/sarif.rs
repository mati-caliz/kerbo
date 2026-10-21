use serde_json::{Value, json};

use crate::i18n::Lang;
use crate::rules::{Diagnostic, RuleId};
use crate::suppressions::Suppressed;

const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";
const INFORMATION_URI: &str = "https://kerbo.matiascaliz.com.ar";
const SOURCE_ROOT: &str = "%SRCROOT%";
const FINGERPRINT_KEY: &str = "kerbo/v1";
const IN_SOURCE: &str = "inSource";

pub(super) fn document(diagnostics: &[Diagnostic], suppressed: &[Suppressed], lang: Lang) -> Value {
    let rules: Vec<Value> = RuleId::ALL
        .iter()
        .map(|rule| {
            json!({
                "id": rule.as_str(),
                "name": rule.as_str(),
                "shortDescription": { "text": lang.describe(*rule) },
                "defaultConfiguration": { "level": "error" },
            })
        })
        .collect();
    json!({
        "$schema": SARIF_SCHEMA,
        "version": SARIF_VERSION,
        "runs": [{
            "tool": { "driver": {
                "name": "kerbo",
                "version": env!("CARGO_PKG_VERSION"),
                "informationUri": INFORMATION_URI,
                "rules": rules,
            }},
            "results": diagnostics
                .iter()
                .map(|diagnostic| result(diagnostic, lang))
                .chain(suppressed.iter().map(|item| suppressed_result(item, lang)))
                .collect::<Vec<Value>>(),
        }],
    })
}

fn result(diagnostic: &Diagnostic, lang: Lang) -> Value {
    let rule_index = RuleId::ALL.iter().position(|rule| *rule == diagnostic.rule);
    json!({
        "ruleId": diagnostic.rule.as_str(),
        "ruleIndex": rule_index,
        "level": "error",
        "message": { "text": lang.message(&diagnostic.message) },
        "locations": [{ "physicalLocation": {
            "artifactLocation": { "uri": diagnostic.relative_path, "uriBaseId": SOURCE_ROOT },
            "region": {
                "startLine": diagnostic.line,
                "startColumn": diagnostic.column,
                "endLine": diagnostic.end_line,
            },
        }}],
        "partialFingerprints": { FINGERPRINT_KEY: diagnostic.fingerprint },
    })
}

fn suppressed_result(item: &Suppressed, lang: Lang) -> Value {
    let mut value = result(&item.diagnostic, lang);
    let suppression = if item.reason.is_empty() {
        json!({ "kind": IN_SOURCE })
    } else {
        json!({ "kind": IN_SOURCE, "justification": item.reason })
    };
    if let Some(fields) = value.as_object_mut() {
        fields.insert("suppressions".to_owned(), json!([suppression]));
    }
    value
}
