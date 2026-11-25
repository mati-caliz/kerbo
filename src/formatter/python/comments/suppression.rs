use crate::formatter::python::trivia::is_python_whitespace;

const COMMENT_START: char = '#';
const FMT_PRAGMA: &str = "fmt:";
const YAPF_PRAGMA: &str = "yapf:";
const SKIP: &str = "skip";

fn trim(text: &str) -> &str {
    text.trim_matches(is_python_whitespace)
}

fn command_after<'text>(text: &'text str, pragma: &str) -> Option<&'text str> {
    text.strip_prefix(pragma)
        .map(|command| command.trim_start_matches(is_python_whitespace))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressionKind {
    Off,
    On,
    Skip,
}

// `SuppressionKind::from_comment` de ruff: `# fmt: off`, `# fmt: on`, `# fmt: skip` (que puede ir
// mezclado con otros, como `# fmt: skip # noqa`), `# yapf: disable` y `# yapf: enable`.
pub fn kind_of(comment: &str) -> Option<SuppressionKind> {
    let trimmed = trim(comment.strip_prefix(COMMENT_START).unwrap_or(comment));
    if let Some(command) = command_after(trimmed, FMT_PRAGMA) {
        match command {
            "off" => return Some(SuppressionKind::Off),
            "on" => return Some(SuppressionKind::On),
            SKIP => return Some(SuppressionKind::Skip),
            _ => {}
        }
    } else if let Some(command) = command_after(trimmed, YAPF_PRAGMA) {
        match command {
            "disable" => return Some(SuppressionKind::Off),
            "enable" => return Some(SuppressionKind::On),
            _ => {}
        }
    }
    comment
        .split(COMMENT_START)
        .any(|segment| command_after(trim(segment), FMT_PRAGMA) == Some(SKIP))
        .then_some(SuppressionKind::Skip)
}

pub fn is_suppression_comment(comment: &str) -> bool {
    kind_of(comment).is_some()
}
