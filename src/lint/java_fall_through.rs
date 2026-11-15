use std::collections::HashSet;

use tree_sitter::Node;

use super::java::text;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

const SWITCH_BLOCK: &str = "switch_block";
const GROUP: &str = "switch_block_statement_group";
const LABEL: &str = "switch_label";
const BLOCK: &str = "block";
const IF: &str = "if_statement";
const FOR: &str = "for_statement";
const ENHANCED_FOR: &str = "enhanced_for_statement";
const WHILE: &str = "while_statement";
const DO: &str = "do_statement";
const TRY: &str = "try_statement";
const TRY_WITH_RESOURCES: &str = "try_with_resources_statement";
const CATCH: &str = "catch_clause";
const FINALLY: &str = "finally_clause";
const SWITCH: &str = "switch_expression";
const SYNCHRONIZED: &str = "synchronized_statement";
const LABELED: &str = "labeled_statement";
const RETURN: &str = "return_statement";
const YIELD: &str = "yield_statement";
const THROW: &str = "throw_statement";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const IDENTIFIER: &str = "identifier";
const TRUE: &str = "true";
const FALSE: &str = "false";
const PARENTHESIZED: &str = "parenthesized_expression";
const COMMENT_SUFFIX: &str = "comment";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const CONDITION_FIELD: &str = "condition";
const BODY_FIELD: &str = "body";
const RELIEF_WORD: &str = "fall";
const RELIEF_SEPARATORS: [char; 2] = [' ', '-'];
const RELIEF_ENDINGS: [&str; 2] = ["thru", "through"];

#[derive(Debug, Clone, Copy)]
struct Exits {
    by_break: bool,
    by_continue: bool,
}

const ANY_EXIT: Exits = Exits {
    by_break: true,
    by_continue: true,
};

// `FallThrough` de Checkstyle: un grupo de `case` con sentencias que no terminan (return, throw,
// break, continue, yield, o un if, try, switch o bucle que terminan en todas sus ramas) y sigue
// otro grupo, sin un comentario "fall through" justo antes del siguiente `case`. El último grupo no
// se mira.
pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    let mut findings: Vec<LintFinding> = nodes
        .iter()
        .filter(|node| node.kind() == SWITCH_BLOCK)
        .flat_map(|block| falls_through(*block, source))
        .collect();
    findings.extend(
        nodes
            .iter()
            .filter(|node| node.kind() == IF && returns_only_booleans(**node, source))
            .map(|statement| {
                finding(
                    LintKind::Java(JavaRule::SimplifyBooleanReturn),
                    *statement,
                    text(*statement, source).lines().next().unwrap_or_default().trim(),
                    None,
                )
            }),
    );
    findings
}

fn falls_through<'tree>(switch_block: Node<'tree>, source: &[u8]) -> Vec<LintFinding> {
    let children: Vec<Node<'tree>> = named_children(switch_block);
    let groups: Vec<(usize, Node<'tree>)> = children
        .iter()
        .enumerate()
        .filter(|(_, child)| child.kind() == GROUP)
        .map(|(index, child)| (index, *child))
        .collect();
    groups
        .windows(2)
        .filter_map(|pair| {
            let [(_, group), (next_index, next)] = pair else {
                return None;
            };
            let statements = statements(*group);
            let last = statements.last()?;
            let next_label = named_children(*next)
                .into_iter()
                .find(|child| child.kind() == LABEL)?;
            if is_terminated(*last, ANY_EXIT, &mut HashSet::new(), source) {
                return None;
            }
            let between = comments_between(*group, &children, *next_index, next_label);
            if has_relief_comment(&between, source) {
                return None;
            }
            Some(finding(
                LintKind::Java(JavaRule::FallThrough),
                next_label,
                text(next_label, source),
                None,
            ))
        })
        .collect()
}

fn statements(group: Node<'_>) -> Vec<Node<'_>> {
    named_children(group)
        .into_iter()
        .filter(|child| child.kind() != LABEL && !child.kind().ends_with(COMMENT_SUFFIX))
        .collect()
}

fn comments_between<'tree>(
    group: Node<'tree>,
    siblings: &[Node<'tree>],
    next_index: usize,
    next_label: Node<'tree>,
) -> Vec<Node<'tree>> {
    let Some(next) = siblings.get(next_index).copied() else {
        return Vec::new();
    };
    let trailing_in_group = named_children(group)
        .into_iter()
        .rev()
        .take_while(|child| child.kind().ends_with(COMMENT_SUFFIX));
    let between_groups = siblings
        .get(..next_index)
        .unwrap_or_default()
        .iter()
        .rev()
        .take_while(|sibling| sibling.kind().ends_with(COMMENT_SUFFIX))
        .copied();
    let leading_in_next = named_children(next)
        .into_iter()
        .take_while(|child| child.id() != next_label.id() && child.kind().ends_with(COMMENT_SUFFIX));
    let mut comments: Vec<Node<'tree>> = trailing_in_group
        .chain(between_groups)
        .chain(leading_in_next)
        .collect();
    comments.sort_by_key(Node::start_byte);
    comments
}

// Checkstyle mira los comentarios que están en la misma línea que el último antes del `case`, y
// el patrón `falls?[ -]?thr(u|ough)` distingue mayúsculas.
fn has_relief_comment(comments: &[Node<'_>], source: &[u8]) -> bool {
    let Some(last) = comments.last() else {
        return false;
    };
    comments
        .iter()
        .filter(|comment| comment.start_position().row == last.start_position().row)
        .any(|comment| mentions_fall_through(text(*comment, source)))
}

fn mentions_fall_through(comment: &str) -> bool {
    comment.match_indices(RELIEF_WORD).any(|(start, _)| {
        let mut rest = comment.get(start + RELIEF_WORD.len()..).unwrap_or_default();
        rest = rest.strip_prefix('s').unwrap_or(rest);
        rest = rest.strip_prefix(RELIEF_SEPARATORS).unwrap_or(rest);
        RELIEF_ENDINGS.iter().any(|ending| rest.starts_with(ending))
    })
}

fn is_terminated(statement: Node<'_>, exits: Exits, labels: &mut HashSet<String>, source: &[u8]) -> bool {
    match statement.kind() {
        RETURN | YIELD | THROW => true,
        BREAK => exits.by_break || has_unknown_label(statement, labels, source),
        CONTINUE => exits.by_continue || has_unknown_label(statement, labels, source),
        BLOCK => last_statement(statement).is_some_and(|last| is_terminated(last, exits, labels, source)),
        IF => if_is_terminated(statement, exits, labels, source),
        FOR | ENHANCED_FOR | WHILE | DO => loop_is_terminated(statement, labels, source),
        TRY | TRY_WITH_RESOURCES => try_is_terminated(statement, exits, labels, source),
        SWITCH => switch_is_terminated(statement, exits, labels, source),
        SYNCHRONIZED => statement
            .child_by_field_name(BODY_FIELD)
            .is_some_and(|body| is_terminated(body, exits, labels, source)),
        LABELED => {
            let children = named_children(statement);
            if let Some(label) = children.first().filter(|child| child.kind() == IDENTIFIER) {
                labels.insert(text(*label, source).to_owned());
            }
            children
                .last()
                .is_some_and(|inner| is_terminated(*inner, exits, labels, source))
        }
        _ => false,
    }
}

fn has_unknown_label(jump: Node<'_>, labels: &HashSet<String>, source: &[u8]) -> bool {
    named_children(jump)
        .into_iter()
        .find(|child| child.kind() == IDENTIFIER)
        .is_some_and(|label| !labels.contains(text(label, source)))
}

fn last_statement(block: Node<'_>) -> Option<Node<'_>> {
    named_children(block)
        .into_iter()
        .rev()
        .find(|child| !child.kind().ends_with(COMMENT_SUFFIX))
}

fn if_is_terminated(statement: Node<'_>, exits: Exits, labels: &mut HashSet<String>, source: &[u8]) -> bool {
    let consequence = statement.child_by_field_name(CONSEQUENCE_FIELD);
    let alternative = statement.child_by_field_name(ALTERNATIVE_FIELD);
    let in_infinite_loop = statement
        .parent()
        .filter(|parent| parent.kind() == BLOCK)
        .and_then(|block| block.parent())
        .is_some_and(|owner| is_infinite_loop(owner, source));
    if in_infinite_loop {
        consequence.is_some_and(|then| is_terminated(then, exits, labels, source))
            || alternative.is_some_and(|other| is_terminated(other, exits, labels, source))
    } else {
        alternative.is_some_and(|other| {
            consequence.is_some_and(|then| is_terminated(then, exits, labels, source))
                && is_terminated(other, exits, labels, source)
        })
    }
}

fn loop_is_terminated(statement: Node<'_>, labels: &mut HashSet<String>, source: &[u8]) -> bool {
    let Some(body) = statement.child_by_field_name(BODY_FIELD) else {
        return false;
    };
    let only_break = Exits {
        by_break: true,
        by_continue: false,
    };
    let none = Exits {
        by_break: false,
        by_continue: false,
    };
    is_infinite_loop(statement, source) && !is_terminated(body, only_break, labels, source)
        || is_terminated(body, none, labels, source)
}

fn is_infinite_loop(statement: Node<'_>, source: &[u8]) -> bool {
    match statement.kind() {
        FOR => statement
            .child_by_field_name(CONDITION_FIELD)
            .is_none_or(|condition| condition.kind() == TRUE),
        WHILE | DO => statement
            .child_by_field_name(CONDITION_FIELD)
            .map(|condition| {
                if condition.kind() == PARENTHESIZED {
                    named_children(condition).into_iter().next().unwrap_or(condition)
                } else {
                    condition
                }
            })
            .is_some_and(|inner| inner.kind() == TRUE && text(inner, source) == TRUE),
        _ => false,
    }
}

fn try_is_terminated(statement: Node<'_>, exits: Exits, labels: &mut HashSet<String>, source: &[u8]) -> bool {
    let children = named_children(statement);
    let finally_ends = children
        .iter()
        .filter(|child| child.kind() == FINALLY)
        .filter_map(|clause| {
            named_children(*clause)
                .into_iter()
                .find(|child| child.kind() == BLOCK)
        })
        .any(|block| is_terminated(block, exits, labels, source));
    if finally_ends {
        return true;
    }
    let body_ends = statement
        .child_by_field_name(BODY_FIELD)
        .is_some_and(|body| is_terminated(body, exits, labels, source));
    body_ends
        && children
            .iter()
            .filter(|child| child.kind() == CATCH)
            .all(|clause| {
                clause
                    .child_by_field_name(BODY_FIELD)
                    .is_some_and(|body| is_terminated(body, exits, labels, source))
            })
}

fn switch_is_terminated(
    statement: Node<'_>,
    exits: Exits,
    labels: &mut HashSet<String>,
    source: &[u8],
) -> bool {
    let groups: Vec<Node<'_>> = statement
        .child_by_field_name(BODY_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|child| !child.kind().ends_with(COMMENT_SUFFIX))
        .collect();
    let inner = Exits {
        by_break: false,
        by_continue: exits.by_continue,
    };
    !groups.is_empty()
        && groups.iter().all(|group| {
            group.kind() == GROUP
                && statements(*group)
                    .last()
                    .is_some_and(|last| is_terminated(*last, inner, labels, source))
        })
}

// `SimplifyBooleanReturn` de Checkstyle: un if con else donde las dos ramas sólo devuelven (o dan
// con yield) un literal booleano, directo o como primera sentencia de un bloque.
fn returns_only_booleans(statement: Node<'_>, source: &[u8]) -> bool {
    let (Some(consequence), Some(alternative)) = (
        statement.child_by_field_name(CONSEQUENCE_FIELD),
        statement.child_by_field_name(ALTERNATIVE_FIELD),
    ) else {
        return false;
    };
    returns_only_a_boolean(consequence, source) && returns_only_a_boolean(alternative, source)
}

fn returns_only_a_boolean(branch: Node<'_>, source: &[u8]) -> bool {
    is_boolean_return(branch, source)
        || branch.kind() == BLOCK
            && named_children(branch)
                .into_iter()
                .find(|child| !child.kind().ends_with(COMMENT_SUFFIX))
                .is_some_and(|first| is_boolean_return(first, source))
}

fn is_boolean_return(statement: Node<'_>, source: &[u8]) -> bool {
    (statement.kind() == RETURN || statement.kind() == YIELD)
        && named_children(statement)
            .into_iter()
            .find(|child| !child.kind().ends_with(COMMENT_SUFFIX))
            .is_some_and(|value| {
                (value.kind() == TRUE || value.kind() == FALSE)
                    && [TRUE, FALSE].contains(&text(value, source))
            })
}
