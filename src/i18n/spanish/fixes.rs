use crate::i18n::catalog::FixNotice;
use crate::rules::RuleId;

pub fn notice(notice: &FixNotice) -> String {
    match notice {
        FixNotice::Applied { count, files } => format!("{count} arreglos aplicados en {files} archivos"),
        FixNotice::Previewed { count, files } => {
            format!("{count} arreglos en {files} archivos; no se escribió nada")
        }
        FixNotice::Withheld { path, introduced } => format!(
            "{path}: después de arreglar aparecen {introduced} problemas nuevos; el archivo quedó como estaba"
        ),
        FixNotice::Left { count } => {
            format!("quedan {count} arreglos que se pisan con otros: revisalos a mano")
        }
    }
}

pub fn suppressed(path: &str, line: usize, rule: RuleId, reason: &str) -> String {
    let reason = if reason.is_empty() { "sin motivo" } else { reason };
    format!("{path}:{line}: {} ignorada en el código: {reason}", rule.as_str())
}

pub fn suppressed_suffix(suppressed: usize) -> String {
    if suppressed > 0 {
        format!(" · {suppressed} ignorados en el código")
    } else {
        String::new()
    }
}
