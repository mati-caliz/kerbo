use tree_sitter::Point;

use crate::rules::ShellRule;
use crate::shell::context::{Context, Finding, Findings};
use crate::shell::syntax::{COMMENT, text};

const SHEBANG: &str = "#!";
const FILE_START: usize = 0;

pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    if context.shell_specified {
        return;
    }
    let has_shebang = context
        .root
        .child(0)
        .filter(|first| first.kind() == COMMENT && first.start_byte() == FILE_START)
        .is_some_and(|first| text(first, context.source).starts_with(SHEBANG));
    if !has_shebang {
        findings.found.push(Finding {
            rule: ShellRule::Sc2148,
            byte: FILE_START,
            point: Point {
                row: FILE_START,
                column: FILE_START,
            },
            subject: String::new(),
            fix: None,
        });
    }
}
