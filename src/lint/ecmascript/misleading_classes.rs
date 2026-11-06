use std::collections::HashSet;

use tree_sitter::Node;

use super::Context;
use super::combining_marks::is_combining_mark;
use super::nodes::{STRING, without_parentheses};
use super::regex_class_sequences::{ClassCharacter, class_sequences, is_surrogate_pair};
use super::regex_code_units::{CodeUnit, from_pattern, from_string_body};
use super::regex_rules::{REGEX, constructions, literal_parts};
use super::regex_syntax::{Mode, parse};
use crate::lint::{LintFinding, LintKind};
use crate::rules::EslintRule;
use crate::syntax::{named_children, one_based};

const TEMPLATE: &str = "template_string";
const TEMPLATE_SUBSTITUTION: &str = "template_substitution";
const FIRST_EMOJI_MODIFIER: u32 = 0x1f3fb;
const LAST_EMOJI_MODIFIER: u32 = 0x1f3ff;
const FIRST_REGIONAL_INDICATOR: u32 = 0x1f1e6;
const LAST_REGIONAL_INDICATOR: u32 = 0x1f1ff;
const ZERO_WIDTH_JOINER: u32 = 0x200d;
const LITERAL_OPENING_LENGTH: usize = 1;
const LINE_FEED: char = '\n';

// `no-misleading-character-class`, en los literales de regex y en `RegExp` con un literal de texto. Lo
// que ESLint resuelve con `getStaticValue` (una constante, una concatenación) no se mira.
pub fn misleading_character_classes(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mut checked_literals = HashSet::new();
    let mut found = Vec::new();
    for construction in constructions(context) {
        let Some(pattern) = construction.pattern.map(without_parentheses) else {
            continue;
        };
        if pattern.kind() == REGEX {
            if construction.flags.is_some() || construction.has_dynamic_flags {
                checked_literals.insert(pattern.id());
            }
            found.extend(construction.flags.as_deref().map_or_else(Vec::new, |flags| {
                regex_literal_findings(pattern, Some(flags), context)
            }));
        } else if !construction.has_dynamic_flags {
            found.extend(text_findings(
                pattern,
                construction.flags.as_deref().unwrap_or_default(),
                context,
            ));
        }
    }
    let literals = context
        .of_kind(&[REGEX])
        .filter(|regex| !checked_literals.contains(&regex.id()));
    for regex in literals {
        found.extend(regex_literal_findings(regex, None, context));
    }
    found
}

fn regex_literal_findings<'tree>(
    regex: Node<'tree>,
    flags: Option<&str>,
    context: &Context<'tree, '_>,
) -> Vec<LintFinding> {
    let (pattern, own_flags) = literal_parts(regex, context);
    let units = from_pattern(&pattern, LITERAL_OPENING_LENGTH);
    findings(regex, &units, flags.unwrap_or(&own_flags), context)
}

fn text_findings(node: Node<'_>, flags: &str, context: &Context<'_, '_>) -> Vec<LintFinding> {
    let is_static_template = node.kind() == TEMPLATE
        && !named_children(node)
            .iter()
            .any(|child| child.kind() == TEMPLATE_SUBSTITUTION);
    if node.kind() != STRING && !is_static_template {
        return Vec::new();
    }
    let raw = context.text(node);
    let body = raw
        .get(LITERAL_OPENING_LENGTH..raw.len().saturating_sub(LITERAL_OPENING_LENGTH))
        .unwrap_or_default();
    let units = from_string_body(body, LITERAL_OPENING_LENGTH);
    findings(node, &units, flags, context)
}

fn findings(node: Node<'_>, units: &[CodeUnit], flags: &str, context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mode = Mode::of_flags(flags);
    let values: Vec<u16> = units.iter().map(|unit| unit.value).collect();
    if parse(&String::from_utf16_lossy(&values), mode).is_err() {
        return Vec::new();
    }
    let raw = context.text(node);
    class_sequences(units, mode)
        .iter()
        .flat_map(|sequence| misleading_starts(sequence))
        .map(|offset| finding_at(node, raw, offset))
        .collect()
}

// Dónde empieza cada coincidencia de `findCharacterSequences`.
fn misleading_starts(sequence: &[ClassCharacter]) -> Vec<usize> {
    let adjacent = || {
        sequence
            .windows(2)
            .filter_map(|pair| Some((*pair.first()?, *pair.get(1)?)))
    };
    let pairs = adjacent()
        .filter(|(previous, character)| is_misleading_pair(*previous, *character))
        .map(|(previous, _)| previous.offset);
    pairs.chain(joined_sequence_starts(sequence)).collect()
}

fn is_misleading_pair(previous: ClassCharacter, character: ClassCharacter) -> bool {
    let is_regional = |value: u32| (FIRST_REGIONAL_INDICATOR..=LAST_REGIONAL_INDICATOR).contains(&value);
    let is_modifier = |value: u32| (FIRST_EMOJI_MODIFIER..=LAST_EMOJI_MODIFIER).contains(&value);
    is_surrogate_pair(previous.value, character.value)
        || (is_combining_mark(character.value) && !is_combining_mark(previous.value))
        || (is_modifier(character.value) && !is_modifier(previous.value))
        || (is_regional(character.value) && is_regional(previous.value))
}

// `zwj`: varias uniones seguidas son una sola coincidencia.
fn joined_sequence_starts(sequence: &[ClassCharacter]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut last_joined: Option<usize> = None;
    for (index, window) in sequence.windows(3).enumerate() {
        let [previous, joiner, next] = window else {
            continue;
        };
        if joiner.value != ZERO_WIDTH_JOINER
            || previous.value == ZERO_WIDTH_JOINER
            || next.value == ZERO_WIDTH_JOINER
        {
            continue;
        }
        if last_joined != Some(index) {
            starts.push(previous.offset);
        }
        last_joined = Some(index.saturating_add(2));
    }
    starts
}

fn finding_at(node: Node<'_>, raw: &str, offset: usize) -> LintFinding {
    let before = raw.get(..offset).unwrap_or_default();
    let start = node.start_position();
    let last_line = before.rsplit(LINE_FEED).next().unwrap_or_default();
    let lines_before = before.matches(LINE_FEED).count();
    let column = if lines_before == 0 {
        start.column.saturating_add(offset)
    } else {
        last_line.len()
    };
    LintFinding {
        kind: LintKind::Eslint(EslintRule::NoMisleadingCharacterClass),
        line: one_based(start.row.saturating_add(lines_before)),
        column: one_based(column),
        subject: String::new(),
        fix: None,
    }
}
