use super::counted;
use crate::i18n::catalog::{FixNotice, SINGLE};
use crate::rules::RuleId;

pub fn notice(notice: &FixNotice) -> String {
    match notice {
        FixNotice::Applied { count, files } => format!(
            "{} en {}",
            counted(*count, "arreglo aplicado", "arreglos aplicados"),
            counted(*files, "archivo", "archivos")
        ),
        FixNotice::Previewed { count, files } => format!(
            "{} en {}; no se escribió nada",
            counted(*count, "arreglo", "arreglos"),
            counted(*files, "archivo", "archivos")
        ),
        FixNotice::Withheld { path, introduced } => format!(
            "{path}: después de arreglar {} {}; el archivo quedó como estaba",
            plural_verb(*introduced, "aparece", "aparecen"),
            counted(*introduced, "problema nuevo", "problemas nuevos")
        ),
        FixNotice::Left { count } => format!(
            "{} {} con otros: revisalos a mano",
            plural_verb(*count, "queda", "quedan"),
            counted(*count, "arreglo que se pisa", "arreglos que se pisan")
        ),
    }
}

const fn plural_verb(count: usize, singular: &'static str, plural: &'static str) -> &'static str {
    if count == SINGLE { singular } else { plural }
}

pub fn suppressed(path: &str, line: usize, rule: RuleId, reason: &str) -> String {
    let reason = if reason.is_empty() { "sin motivo" } else { reason };
    format!("{path}:{line}: {} ignorada en el código: {reason}", rule.as_str())
}

pub fn suppressed_suffix(suppressed: usize) -> String {
    if suppressed > 0 {
        format!(" · {} en el código", counted(suppressed, "ignorado", "ignorados"))
    } else {
        String::new()
    }
}
