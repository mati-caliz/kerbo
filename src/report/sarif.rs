use serde_json::{Value, json};

use crate::rules::{Diagnostic, RuleId};

const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";
const INFORMATION_URI: &str = "https://kerbo.matiascaliz.com.ar";
const SOURCE_ROOT: &str = "%SRCROOT%";
const FINGERPRINT_KEY: &str = "kerbo/v1";

pub(super) fn document(diagnostics: &[Diagnostic]) -> Value {
    let rules: Vec<Value> = RuleId::ALL
        .iter()
        .map(|rule| {
            json!({
                "id": rule.as_str(),
                "name": rule.as_str(),
                "shortDescription": { "text": rule.description() },
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
            "results": diagnostics.iter().map(result).collect::<Vec<Value>>(),
        }],
    })
}

fn result(diagnostic: &Diagnostic) -> Value {
    let rule_index = RuleId::ALL.iter().position(|rule| *rule == diagnostic.rule);
    json!({
        "ruleId": diagnostic.rule.as_str(),
        "ruleIndex": rule_index,
        "level": "error",
        "message": { "text": diagnostic.message },
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
