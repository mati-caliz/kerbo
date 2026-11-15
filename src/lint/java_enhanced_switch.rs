use tree_sitter::Node;

use super::java::text;
use super::java_switch::yields_value;
use super::java_switch_assignment::assigns_in_every_branch;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const ENHANCED_SWITCH_RELEASE: u32 = 14;
const SWITCH: &str = "switch_expression";
const SWITCH_KEYWORD: &str = "switch";
const GROUP: &str = "switch_block_statement_group";
const LABEL: &str = "switch_label";
const BLOCK: &str = "block";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const YIELD: &str = "yield_statement";
const RETURN: &str = "return_statement";
const THROW: &str = "throw_statement";
const BODY_FIELD: &str = "body";
const COMMENT_SUFFIX: &str = "comment";
const DEFAULT: &str = "default";
const MAX_STATEMENTS_IN_BRANCH: usize = 2;
const JUMPS: [&str; 3] = [BREAK, CONTINUE, YIELD];

// `EnhancedSwitchMigration` de IntelliJ en la forma que avisa por defecto: un switch viejo que
// devuelve (o tira) en todas sus ramas y se puede escribir como `return switch (...) { case X -> ... }`.
// Cada rama, de hasta dos sentencias, termina en return con valor o en throw, sin break, continue
// ni yield adentro; sin `default`, tiene que seguirle un return o un throw que hace de default.
pub fn reported(nodes: &[Node<'_>], source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if release.is_some_and(|release| release < ENHANCED_SWITCH_RELEASE) {
        return Vec::new();
    }
    nodes
        .iter()
        .filter(|node| node.kind() == SWITCH && !yields_value(**node))
        .filter(|switch| {
            returns_in_every_branch(**switch, source) || assigns_in_every_branch(**switch, source)
        })
        .filter_map(|switch| {
            let keyword = switch
                .child(0)
                .filter(|keyword| keyword.kind() == SWITCH_KEYWORD)?;
            Some(finding(
                LintKind::Java(JavaRule::EnhancedSwitchMigration),
                keyword,
                SWITCH_KEYWORD,
                None,
            ))
        })
        .collect()
}

fn returns_in_every_branch(switch: Node<'_>, source: &[u8]) -> bool {
    let Some(body) = switch.child_by_field_name(BODY_FIELD) else {
        return false;
    };
    let entries: Vec<Node<'_>> = named_children(body)
        .into_iter()
        .filter(|entry| !entry.kind().ends_with(COMMENT_SUFFIX))
        .collect();
    if entries.is_empty() || entries.iter().any(|entry| entry.kind() != GROUP) {
        return false;
    }
    let exits_after = switch
        .next_named_sibling()
        .filter(|next| !next.kind().ends_with(COMMENT_SUFFIX))
        .is_some_and(|next| next.kind() == RETURN || next.kind() == THROW);
    let exhaustive = entries.iter().any(|group| has_default(*group, source));
    if !exhaustive && !exits_after {
        return false;
    }
    let mut returns_something = false;
    for (index, group) in entries.iter().enumerate() {
        let is_last = index + 1 == entries.len();
        let statements = branch_statements(*group);
        if statements.is_empty() {
            let falls_into_next =
                !is_last && named_children(*group).iter().all(|child| child.kind() == LABEL);
            if !falls_into_next && !exits_after {
                return false;
            }
            continue;
        }
        let Some(last) = statements.last() else {
            return false;
        };
        let has_jump = statements.iter().any(|statement| {
            descendants(*statement)
                .iter()
                .any(|part| JUMPS.contains(&part.kind()))
        });
        if statements.len() > MAX_STATEMENTS_IN_BRANCH || has_jump {
            return false;
        }
        match last.kind() {
            RETURN
                if named_children(*last)
                    .iter()
                    .any(|value| !value.kind().ends_with(COMMENT_SUFFIX)) =>
            {
                returns_something = true;
            }
            THROW => {}
            _ => return false,
        }
    }
    returns_something
}

// Las sentencias de una rama, hasta un `break` del propio switch (lo que sigue no se ejecuta); si la
// rama es un único bloque, las de adentro.
pub(super) fn branch_statements(group: Node<'_>) -> Vec<Node<'_>> {
    let mut statements: Vec<Node<'_>> = named_children(group)
        .into_iter()
        .filter(|child| child.kind() != LABEL && !child.kind().ends_with(COMMENT_SUFFIX))
        .take_while(|statement| statement.kind() != BREAK)
        .collect();
    if let [only] = statements[..]
        && only.kind() == BLOCK
    {
        statements = named_children(only)
            .into_iter()
            .filter(|statement| !statement.kind().ends_with(COMMENT_SUFFIX))
            .collect();
    }
    statements
}

pub(super) fn has_default(group: Node<'_>, source: &[u8]) -> bool {
    named_children(group)
        .into_iter()
        .filter(|child| child.kind() == LABEL)
        .any(|label| {
            text(label, source)
                .split_whitespace()
                .any(|word| word.trim_end_matches(':') == DEFAULT)
        })
}
