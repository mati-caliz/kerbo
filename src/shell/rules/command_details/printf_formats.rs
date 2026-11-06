use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::Findings;
use crate::shell::syntax::{COMMAND, text};
use crate::shell::word_expansion::may_become_multiple;
use crate::shell::word_pieces::{literal_string, pieces};

const PRINTF: &str = "printf";
const END_OF_OPTIONS: &str = "--";
const VARIABLE_OPTION: &str = "-v";
const PERCENT: char = '%';
const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const STAR: char = '*';
const TIME_FORMAT: char = 'T';
const LENGTH_MODIFIERS: &[&str] = &["hh", "ll", "h", "l", "q", "L", "j", "z", "Z", "t"];
const FORMAT_CHARACTERS: &str = "diouxXfFeEgGaAcsbqQSC";

// SC2182 y SC2183: `checkPrintfVar`.
pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if node.kind() != COMMAND {
        return;
    }
    let Some(command) = Command::of(node).filter(|command| command.is_exactly(PRINTF, source)) else {
        return;
    };
    let mut arguments = command.effective_arguments(source);
    loop {
        let literal = arguments
            .first()
            .and_then(|first| literal_string(&pieces(*first, source)));
        match literal.as_deref() {
            Some(END_OF_OPTIONS) => arguments = arguments.get(1..).unwrap_or_default(),
            Some(VARIABLE_OPTION) if arguments.len() >= 2 => {
                arguments = arguments.get(2..).unwrap_or_default();
            }
            _ => break,
        }
    }
    let Some((format, more)) = arguments.split_first() else {
        return;
    };
    let Some(format_text) = literal_string(&pieces(*format, source)) else {
        return;
    };
    let formats = printf_formats(&format_text);
    let is_multiple = more
        .iter()
        .any(|argument| may_become_multiple(&pieces(*argument, source)));
    if let Some(rule) = argument_rule(&formats, more.len(), is_multiple) {
        findings.report(rule, *format, text(*format, source));
    }
}

fn argument_rule(formats: &[char], arguments: usize, is_multiple: bool) -> Option<ShellRule> {
    let wanted = formats.len();
    if arguments == 0 && wanted == 0 {
        return None;
    }
    if wanted == 0 {
        return Some(ShellRule::Sc2182);
    }
    let only_trailing_times = || {
        formats
            .iter()
            .skip(arguments)
            .all(|format| *format == TIME_FORMAT)
    };
    let is_fine = is_multiple
        || (arguments < wanted && only_trailing_times())
        || (arguments > 0 && arguments.is_multiple_of(wanted));
    (!is_fine).then_some(ShellRule::Sc2183)
}

// `getPrintfFormats`: "Hello %s" da `s` y "%(%s)T %0*d\n" da `T*d`.
fn printf_formats(format: &str) -> Vec<char> {
    let mut found = Vec::new();
    let mut rest = format;
    while let Some(index) = rest.find(PERCENT) {
        let after = rest
            .get(index.saturating_add(PERCENT.len_utf8())..)
            .unwrap_or_default();
        if let Some(following) = after.strip_prefix(PERCENT) {
            rest = following;
        } else if let Some(time) = after.strip_prefix(OPEN_PARENTHESIS) {
            let Some((_, trailing)) = time.split_once(CLOSE_PARENTHESIS) else {
                return found;
            };
            let mut characters = trailing.chars();
            let Some(kind) = characters.next() else {
                return found;
            };
            found.push(kind);
            rest = characters.as_str();
        } else if let Some((types, consumed)) = parse_specifier(after) {
            found.extend(types);
            rest = after.get(consumed..).unwrap_or_default();
        } else {
            found.extend(after.chars().next());
            rest = after;
        }
    }
    found
}

fn skip_optional(text: &str, position: &mut usize, character: char) {
    if text
        .get(*position..)
        .is_some_and(|rest| rest.starts_with(character))
    {
        *position = position.saturating_add(character.len_utf8());
    }
}

fn digits_or_star(text: &str, position: &mut usize) -> bool {
    let rest = text.get(*position..).unwrap_or_default();
    if rest.starts_with(STAR) {
        *position = position.saturating_add(STAR.len_utf8());
        return true;
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    *position = position.saturating_add(digits);
    false
}

// La expresión `^#?-?\+? ?0?(\*|\d*)\.?(\d*|\*)(hh|h|l|ll|q|L|j|z|Z|t)?([diouxXfFeEgGaAcsbqQSC])`.
fn parse_specifier(text: &str) -> Option<(Vec<char>, usize)> {
    let mut position = 0;
    for flag in ['#', '-', '+', ' ', '0'] {
        skip_optional(text, &mut position, flag);
    }
    let mut types = Vec::new();
    if digits_or_star(text, &mut position) {
        types.push(STAR);
    }
    skip_optional(text, &mut position, '.');
    if digits_or_star(text, &mut position) {
        types.push(STAR);
    }
    let rest = text.get(position..)?;
    let with_length = LENGTH_MODIFIERS.iter().find_map(|modifier| {
        let after = rest.strip_prefix(modifier)?;
        let kind = after
            .chars()
            .next()
            .filter(|kind| FORMAT_CHARACTERS.contains(*kind))?;
        Some((kind, modifier.len()))
    });
    let without_length = || {
        rest.chars()
            .next()
            .filter(|kind| FORMAT_CHARACTERS.contains(*kind))
            .map(|kind| (kind, 0))
    };
    let (kind, skipped) = with_length.or_else(without_length)?;
    types.push(kind);
    Some((
        types,
        position.saturating_add(skipped).saturating_add(kind.len_utf8()),
    ))
}
