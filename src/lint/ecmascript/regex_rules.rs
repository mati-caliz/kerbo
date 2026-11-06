use tree_sitter::Node;

use super::constants::Constants;
use super::nodes::{ARGUMENTS_FIELD, CALL, FUNCTION_FIELD, IDENTIFIER, STRING, without_parentheses};
use super::regex_syntax::{Kind, Mode, RegexTree, parse};
use super::strings::cooked;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

pub const REGEX: &str = "regex";
const PATTERN_FIELD: &str = "pattern";
const FLAGS_FIELD: &str = "flags";
const NEW: &str = "new_expression";
const CONSTRUCTOR_FIELD: &str = "constructor";
const REG_EXP: &str = "RegExp";
const VALID_FLAGS: &str = "dgimsuvy";
const UNICODE_FLAG: char = 'u';
const UNICODE_SETS_FLAG: char = 'v';
const COMMENT: &str = "comment";
const DOUBLE_SPACE: &str = "  ";
const SPACE: char = ' ';
const QUANTIFIERS: &[char] = &['+', '*', '{', '?'];
const MINIMUM_SPACES: usize = 2;
const STRICT_MODES: [Mode; 3] = [
    Mode {
        unicode: true,
        unicode_sets: false,
    },
    Mode {
        unicode: false,
        unicode_sets: true,
    },
    Mode {
        unicode: false,
        unicode_sets: false,
    },
];

pub struct Constructed<'tree> {
    pub node: Node<'tree>,
    pub pattern: Option<Node<'tree>>,
    pub flags: Option<String>,
    pub has_dynamic_flags: bool,
}

fn string_value(node: Node<'_>, context: &Context<'_, '_>) -> Option<String> {
    let node = without_parentheses(node);
    if node.kind() != STRING {
        return None;
    }
    let raw = context.text(node);
    Some(cooked(raw.get(1..raw.len().saturating_sub(1))?))
}

// Las llamadas a `RegExp` y `new RegExp` del global, con sus argumentos.
pub fn constructions<'tree>(context: &Context<'tree, '_>) -> Vec<Constructed<'tree>> {
    let constants = Constants::new(context);
    context
        .of_kind(&[CALL, NEW])
        .filter_map(|call| {
            let callee = call
                .child_by_field_name(FUNCTION_FIELD)
                .or_else(|| call.child_by_field_name(CONSTRUCTOR_FIELD))?;
            if callee.kind() != IDENTIFIER || context.text(callee) != REG_EXP || !constants.is_global(callee)
            {
                return None;
            }
            let arguments: Vec<Node<'tree>> =
                call.child_by_field_name(ARGUMENTS_FIELD)
                    .map_or_else(Vec::new, |list| {
                        (0..list.named_child_count())
                            .filter_map(|index| {
                                u32::try_from(index)
                                    .ok()
                                    .and_then(|index| list.named_child(index))
                            })
                            .filter(|argument| argument.kind() != COMMENT)
                            .collect()
                    });
            let flags_node = arguments.get(1).copied();
            let flags = flags_node.and_then(|flags| string_value(flags, context));
            Some(Constructed {
                node: call,
                pattern: arguments.first().copied(),
                has_dynamic_flags: flags_node.is_some() && flags.is_none(),
                flags,
            })
        })
        .collect()
}

pub fn literal_parts<'tree>(regex: Node<'tree>, context: &Context<'tree, '_>) -> (String, String) {
    let part = |field: &str| {
        regex
            .child_by_field_name(field)
            .map_or_else(String::new, |found| context.text(found).to_owned())
    };
    (part(PATTERN_FIELD), part(FLAGS_FIELD))
}

// `no-invalid-regexp`.
pub fn invalid_regexes(context: &Context<'_, '_>) -> Vec<LintFinding> {
    constructions(context)
        .into_iter()
        .filter(|construction| is_invalid(construction, context))
        .map(|construction| report(EslintRule::NoInvalidRegexp, construction.node, ""))
        .collect()
}

fn is_invalid(construction: &Constructed<'_>, context: &Context<'_, '_>) -> bool {
    let flags = construction.flags.as_deref().unwrap_or_default();
    let mut remaining = flags.to_owned();
    for flag in VALID_FLAGS.chars() {
        remaining = remaining.replacen(flag, "", 1);
    }
    if !remaining.is_empty() || (flags.contains(UNICODE_FLAG) && flags.contains(UNICODE_SETS_FLAG)) {
        return true;
    }
    let Some(pattern) = construction
        .pattern
        .and_then(|pattern| string_value(pattern, context))
    else {
        return false;
    };
    if construction.has_dynamic_flags {
        return STRICT_MODES.iter().all(|mode| parse(&pattern, *mode).is_err());
    }
    parse(&pattern, Mode::of_flags(flags)).is_err()
}

// `no-empty-character-class`: sólo en los literales.
pub fn empty_character_classes(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[REGEX])
        .flat_map(|regex| {
            let (pattern, flags) = literal_parts(regex, context);
            let empties = parse(&pattern, Mode::of_flags(&flags)).map_or(0, |tree| {
                tree.nodes
                    .iter()
                    .filter(|node| {
                        matches!(
                            node.kind,
                            Kind::Class {
                                negate: false,
                                empty: true
                            }
                        )
                    })
                    .count()
            });
            std::iter::repeat_n(report(EslintRule::NoEmptyCharacterClass, regex, ""), empties)
        })
        .collect()
}

// `no-regex-spaces`.
pub fn repeated_regex_spaces(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let literals = context.of_kind(&[REGEX]).map(|regex| {
        let (pattern, flags) = literal_parts(regex, context);
        (regex, pattern, flags)
    });
    let constructed = constructions(context).into_iter().filter_map(|construction| {
        let raw_ok = construction
            .pattern
            .map(without_parentheses)
            .filter(|pattern| pattern.kind() == STRING)
            .is_some_and(|pattern| context.text(pattern).contains(DOUBLE_SPACE));
        if !raw_ok || construction.has_dynamic_flags {
            return None;
        }
        let pattern = string_value(construction.pattern?, context)?;
        Some((construction.node, pattern, construction.flags.unwrap_or_default()))
    });
    literals
        .chain(constructed)
        .filter(|(_, pattern, flags)| has_repeated_spaces(pattern, Mode::of_flags(flags)))
        .map(|(node, _, _)| report(EslintRule::NoRegexSpaces, node, ""))
        .collect()
}

// La expresión `( {2,})(?: [+*{?]|[^+*{?]|$)` de ESLint, afuera de una clase de caracteres.
fn has_repeated_spaces(pattern: &str, mode: Mode) -> bool {
    if !pattern.contains(DOUBLE_SPACE) {
        return false;
    }
    let Ok(tree) = parse(pattern, mode) else {
        return false;
    };
    let is_in_class = |index: usize| {
        tree.nodes
            .iter()
            .any(|node| matches!(node.kind, Kind::Class { .. }) && node.start <= index && index < node.end)
    };
    let mut index = 0;
    while let Some(start) = pattern
        .get(index..)
        .and_then(|rest| rest.find(DOUBLE_SPACE))
        .map(|found| found.saturating_add(index))
    {
        let rest = pattern.get(start..).unwrap_or_default();
        let run = rest.chars().take_while(|character| *character == SPACE).count();
        let after = rest.get(run..).unwrap_or_default();
        let counted = if after.starts_with(QUANTIFIERS) {
            run.saturating_sub(1)
        } else {
            run
        };
        if counted >= MINIMUM_SPACES && !is_in_class(start) {
            return true;
        }
        index = start.saturating_add(run);
    }
    false
}

// `no-useless-backreference`.
pub fn useless_backreferences(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let literals = context.of_kind(&[REGEX]).map(|regex| {
        let (pattern, flags) = literal_parts(regex, context);
        (regex, pattern, flags)
    });
    let constructed = constructions(context).into_iter().filter_map(|construction| {
        let pattern = string_value(construction.pattern?, context)?;
        Some((construction.node, pattern, construction.flags.unwrap_or_default()))
    });
    literals
        .chain(constructed)
        .flat_map(|(node, pattern, flags)| {
            let count = parse(&pattern, Mode::of_flags(&flags)).map_or(0, |tree| count_useless(&tree));
            std::iter::repeat_n(report(EslintRule::NoUselessBackreference, node, ""), count)
        })
        .collect()
}

fn count_useless(tree: &RegexTree) -> usize {
    tree.nodes
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            let Kind::Backreference(reference) = &node.kind else {
                return false;
            };
            let groups = tree.resolved(reference);
            !groups.is_empty()
                && groups
                    .iter()
                    .all(|group| is_useless_reference(tree, *index, *group))
        })
        .count()
}

fn is_lookaround(tree: &RegexTree, node: usize) -> Option<(bool, bool)> {
    match tree.nodes.get(node).map(|found| &found.kind) {
        Some(Kind::Look { behind, negate }) => Some((*behind, *negate)),
        _ => None,
    }
}

// Los motivos de ESLint: anidada, en otra alternativa, adelantada, atrasada o adentro de una negación.
fn is_useless_reference(tree: &RegexTree, reference: usize, group: usize) -> bool {
    let reference_path = tree.path(reference);
    if reference_path.contains(&group) {
        return true;
    }
    let group_path = tree.path(group);
    let shared = reference_path
        .iter()
        .rev()
        .zip(group_path.iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let cut_length = group_path.len().saturating_sub(shared);
    let (cut, common) = group_path.split_at(cut_length);
    let is_matching_backward = common
        .iter()
        .find_map(|node| is_lookaround(tree, *node))
        .is_some_and(|(behind, _)| behind);
    let is_disjunctive = cut
        .last()
        .and_then(|node| tree.nodes.get(*node))
        .is_some_and(|node| node.kind == Kind::Alternative);
    let (Some(reference_node), Some(group_node)) = (tree.nodes.get(reference), tree.nodes.get(group)) else {
        return false;
    };
    is_disjunctive
        || (!is_matching_backward && reference_node.end <= group_node.start)
        || (is_matching_backward && group_node.end <= reference_node.start)
        || cut
            .iter()
            .any(|node| is_lookaround(tree, *node).is_some_and(|(_, negate)| negate))
}
