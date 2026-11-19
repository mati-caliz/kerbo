use crate::formatter::python::trivia::is_python_whitespace;

const COMMENT_START: char = '#';
const FMT_PRAGMA: &str = "fmt:";
const YAPF_PRAGMA: &str = "yapf:";
const FMT_COMMANDS: [&str; 3] = ["off", "on", "skip"];
const YAPF_COMMANDS: [&str; 2] = ["disable", "enable"];
const SKIP: &str = "skip";

fn trim(text: &str) -> &str {
    text.trim_matches(is_python_whitespace)
}

fn command_after<'text>(text: &'text str, pragma: &str) -> Option<&'text str> {
    text.strip_prefix(pragma)
        .map(|command| command.trim_start_matches(is_python_whitespace))
}

// `SuppressionKind::from_comment` de ruff: `# fmt: off`, `# fmt: on`, `# fmt: skip` (que puede ir
// mezclado con otros, como `# fmt: skip # noqa`), `# yapf: disable` y `# yapf: enable`.
pub fn is_suppression_comment(comment: &str) -> bool {
    let trimmed = trim(comment.strip_prefix(COMMENT_START).unwrap_or(comment));
    let is_whole_comment = command_after(trimmed, FMT_PRAGMA)
        .is_some_and(|command| FMT_COMMANDS.contains(&command))
        || command_after(trimmed, YAPF_PRAGMA).is_some_and(|command| YAPF_COMMANDS.contains(&command));
    is_whole_comment
        || comment
            .split(COMMENT_START)
            .any(|segment| command_after(trim(segment), FMT_PRAGMA) == Some(SKIP))
}
