use tree_sitter::{Node, Point};

use super::{LintFinding, LintKind, finding_at};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::descendants;

const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const TAB: u8 = b'\t';
const SPACE: u8 = b' ';
const FORM_FEED: u8 = 0x0c;
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const TODO_MARK: &str = "TODO:";
const COMMENT_OPENER_WIDTH: usize = 2;
const LINE_ONLY_COLUMN: usize = 0;

// Lo que Checkstyle mira sobre el texto y no sobre el árbol: un comentario con `TODO:`, el primer
// tab del archivo, los espacios al final de cada línea y el salto de línea final.
pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let mut findings: Vec<LintFinding> = descendants(root)
        .into_iter()
        .filter(|node| COMMENTS.contains(&node.kind()))
        .filter_map(|comment| todo(comment, source))
        .collect();
    findings.extend(first_tab(source));
    findings.extend(trailing_spaces(source));
    findings.extend(missing_final_newline(source));
    findings
}

fn todo(comment: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let text = comment.utf8_text(source).ok()?;
    text.get(COMMENT_OPENER_WIDTH..)?.contains(TODO_MARK).then(|| {
        let start = comment.start_position();
        let position = Point::new(start.row, start.column.saturating_add(COMMENT_OPENER_WIDTH));
        finding_at(LintKind::Java(JavaRule::TodoComment), position, "", None)
    })
}

fn lines(source: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    source.split(|byte| *byte == LINE_FEED).enumerate()
}

fn first_tab(source: &[u8]) -> Option<LintFinding> {
    lines(source).find_map(|(row, line)| {
        let column = line.iter().position(|byte| *byte == TAB)?;
        Some(finding_at(
            LintKind::Java(JavaRule::FileTabCharacter),
            Point::new(row, column),
            "",
            None,
        ))
    })
}

fn trailing_spaces(source: &[u8]) -> Vec<LintFinding> {
    lines(source)
        .filter_map(|(row, line)| {
            let content = line.strip_suffix(&[CARRIAGE_RETURN]).unwrap_or(line);
            let kept = content
                .iter()
                .rposition(|byte| !is_blank(*byte))
                .map_or(0, |last| last.saturating_add(1));
            (kept < content.len()).then(|| {
                let fix =
                    Fix::replace_range(Point::new(row, kept), Point::new(row, content.len()), "", source);
                finding_at(
                    LintKind::Java(JavaRule::TrailingSpaces),
                    Point::new(row, LINE_ONLY_COLUMN),
                    "",
                    Some(fix),
                )
            })
        })
        .collect()
}

const fn is_blank(byte: u8) -> bool {
    matches!(byte, SPACE | TAB | FORM_FEED | CARRIAGE_RETURN)
}

fn missing_final_newline(source: &[u8]) -> Option<LintFinding> {
    let last = *source.last()?;
    if last == LINE_FEED || last == CARRIAGE_RETURN {
        return None;
    }
    let rows = lines(source).count().saturating_sub(1);
    let end_column = source.iter().rev().take_while(|byte| **byte != LINE_FEED).count();
    let fix = Fix::insert_at(Point::new(rows, end_column), "\n", source);
    Some(finding_at(
        LintKind::Java(JavaRule::NewlineAtEndOfFile),
        Point::new(0, LINE_ONLY_COLUMN),
        "",
        Some(fix),
    ))
}
