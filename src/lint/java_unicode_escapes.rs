mod kept;

use tree_sitter::{Node, Point};

use self::kept::KEPT_RANGES;
use super::{LintFinding, LintKind, finding_at};
use crate::fix::Fix;
use crate::limits::SourceEncoding;
use crate::rules::JavaRule;

const BACKSLASH: u8 = b'\\';
const ESCAPE_MARK: u8 = b'u';
const LINE_FEED: u8 = b'\n';
const HEX_DIGITS: usize = 4;
const HEX_RADIX: u32 = 16;
const TAB: char = '\t';
const ESCAPED_TAB: &str = "\\t";
const STRING_LITERAL: &str = "string_literal";

pub fn reported(root: Node<'_>, source: &[u8], project_encoding: SourceEncoding) -> Vec<LintFinding> {
    let encoding = file_encoding(source, project_encoding);
    let mut findings = Vec::new();
    let mut row = 0;
    let mut line_start = 0;
    let mut backslashes_before = 0_usize;
    for (offset, &byte) in source.iter().enumerate() {
        if byte == LINE_FEED {
            row += 1;
            line_start = offset + 1;
        }
        if byte != BACKSLASH {
            backslashes_before = 0;
            continue;
        }
        let is_escape = backslashes_before.is_multiple_of(2);
        backslashes_before += 1;
        if !is_escape {
            continue;
        }
        let Some((digits, character)) = unnecessary_escape(source, offset, encoding) else {
            continue;
        };
        let end = digits + HEX_DIGITS;
        let start = Point::new(row, offset - line_start);
        let end_point = Point::new(row, end - line_start);
        let escape = String::from_utf8_lossy(source.get(offset..end).unwrap_or_default());
        let replacement = replacement(root, offset, character);
        findings.push(finding_at(
            LintKind::Java(JavaRule::UnnecessaryUnicodeEscape),
            start,
            &escape,
            Some(Fix::replace_range(start, end_point, &replacement, source)),
        ));
    }
    findings
}

// Como IntelliJ: un archivo que ya trae UTF-8 válido fuera de ASCII se lee como UTF-8 aunque el
// proyecto declare otra codificación; uno ASCII puro, con la del proyecto.
const fn file_encoding(source: &[u8], project_encoding: SourceEncoding) -> SourceEncoding {
    if !source.is_ascii() && std::str::from_utf8(source).is_ok() {
        SourceEncoding::Utf8
    } else {
        project_encoding
    }
}

fn unnecessary_escape(source: &[u8], backslash: usize, encoding: SourceEncoding) -> Option<(usize, char)> {
    let digits = escape_digits(source, backslash)?;
    let code = escape_value(source, digits)?;
    let follows_another_escape =
        code == u16::from(BACKSLASH) && escape_digits(source, digits + HEX_DIGITS - 1).is_some();
    if follows_another_escape || is_kept(code) || !encoding.can_write(code) {
        return None;
    }
    Some((digits, char::from_u32(u32::from(code))?))
}

fn escape_digits(source: &[u8], backslash: usize) -> Option<usize> {
    let first_mark = backslash + 1;
    let digits = source
        .get(first_mark..)?
        .iter()
        .position(|&byte| byte != ESCAPE_MARK)
        .map_or(source.len(), |marks| first_mark + marks);
    let complete = digits + HEX_DIGITS <= source.len();
    (digits > first_mark && complete).then_some(digits)
}

fn escape_value(source: &[u8], digits: usize) -> Option<u16> {
    let hex = source.get(digits..digits + HEX_DIGITS)?;
    if !hex.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    u16::from_str_radix(std::str::from_utf8(hex).ok()?, HEX_RADIX).ok()
}

fn is_kept(code: u16) -> bool {
    KEPT_RANGES
        .as_chunks::<2>()
        .0
        .iter()
        .any(|[first, last]| (*first..=*last).contains(&code))
}

fn replacement(root: Node<'_>, offset: usize, character: char) -> String {
    let in_string = character == TAB
        && root
            .descendant_for_byte_range(offset, offset)
            .is_some_and(|node| {
                std::iter::successors(Some(node), Node::parent)
                    .any(|ancestor| ancestor.kind() == STRING_LITERAL)
            });
    if in_string {
        ESCAPED_TAB.to_owned()
    } else {
        character.to_string()
    }
}
