use super::counted;
use crate::formatter::Unparsable;
use crate::i18n::catalog::{FixNotice, FormatNotice};
use crate::rules::RuleId;

pub fn notice(notice: &FixNotice) -> String {
    match notice {
        FixNotice::Applied { count, files } => format!(
            "{} applied in {}",
            counted(*count, "fix", "fixes"),
            counted(*files, "file", "files")
        ),
        FixNotice::Previewed { count, files } => format!(
            "{} in {}; nothing was written",
            counted(*count, "fix", "fixes"),
            counted(*files, "file", "files")
        ),
        FixNotice::Withheld { path, introduced } => format!(
            "{path}: after fixing, {} show up; the file was left as it was",
            counted(*introduced, "new problem", "new problems")
        ),
        FixNotice::Left { count } => format!(
            "{} left that overlap with others: review them by hand",
            counted(*count, "fix", "fixes")
        ),
    }
}

pub fn format_notice(notice: &FormatNotice) -> String {
    match notice {
        FormatNotice::Written { files, total } => format!(
            "{} of {total}",
            counted(*files, "file formatted", "files formatted")
        ),
        FormatNotice::Unformatted { files, total } => {
            format!(
                "{} of {total} not formatted like gofmt",
                counted(*files, "file", "files")
            )
        }
        FormatNotice::Unparsable { path, cause } => match cause {
            Unparsable::SyntaxError { line } => {
                format!("{path}:{line}: has a syntax error; left as it was")
            }
            Unparsable::ReadsUnlikeGo { line } => {
                format!("{path}:{line}: tree-sitter-go reads it differently than Go; left as it was")
            }
            Unparsable::Unsupported { line } => {
                format!("{path}:{line}: kerbo does not format this syntax yet; left as it was")
            }
            Unparsable::UnsupportedSyntax { line, syntax } => {
                format!("{path}:{line}: kerbo does not format `{syntax}` yet; left as it was")
            }
        },
    }
}

pub fn suppressed(path: &str, line: usize, rule: RuleId, reason: &str) -> String {
    let reason = if reason.is_empty() {
        "no reason given"
    } else {
        reason
    };
    format!("{path}:{line}: {} ignored in the code: {reason}", rule.as_str())
}

pub fn suppressed_suffix(suppressed: usize) -> String {
    if suppressed > 0 {
        format!(
            " · {} in the code",
            counted(suppressed, "ignored problem", "ignored problems")
        )
    } else {
        String::new()
    }
}
