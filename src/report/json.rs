use serde::Serialize;

use crate::i18n::Lang;
use crate::rules::{Diagnostic, RuleId};

#[derive(Debug, Serialize)]
pub(super) struct JsonDiagnostic<'diagnostic> {
    path: &'diagnostic str,
    line: usize,
    column: usize,
    end_line: usize,
    rule: RuleId,
    message: String,
    value: usize,
    limit: usize,
    fingerprint: &'diagnostic str,
}

pub(super) fn document(diagnostics: &[Diagnostic], lang: Lang) -> Vec<JsonDiagnostic<'_>> {
    diagnostics
        .iter()
        .map(|diagnostic| JsonDiagnostic {
            path: &diagnostic.path,
            line: diagnostic.line,
            column: diagnostic.column,
            end_line: diagnostic.end_line,
            rule: diagnostic.rule,
            message: lang.message(&diagnostic.message),
            value: diagnostic.value,
            limit: diagnostic.limit,
            fingerprint: &diagnostic.fingerprint,
        })
        .collect()
}
