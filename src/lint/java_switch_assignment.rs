use tree_sitter::Node;

use super::java::{IDENTIFIER, NAME_FIELD, VARIABLE_DECLARATOR, text};
use super::java_enhanced_switch::{branch_statements, has_default};
use crate::syntax::{descendants, named_children};

const GROUP: &str = "switch_block_statement_group";
const LABEL: &str = "switch_label";
const BLOCK: &str = "block";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const YIELD: &str = "yield_statement";
const RETURN: &str = "return_statement";
const THROW: &str = "throw_statement";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const ASSIGNMENT: &str = "assignment_expression";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const BODY_FIELD: &str = "body";
const LEFT_FIELD: &str = "left";
const OPERATOR_FIELD: &str = "operator";
const VALUE_FIELD: &str = "value";
const COMMENT_SUFFIX: &str = "comment";
const ASSIGN: &str = "=";
const MAX_STATEMENTS_IN_BRANCH: usize = 2;
const EXITS: [&str; 3] = [CONTINUE, YIELD, RETURN];
const SCOPE_OWNERS: [&str; 4] = [
    "method_declaration",
    "constructor_declaration",
    "lambda_expression",
    "class_body",
];

// La otra forma de `EnhancedSwitchMigration` que avisa por defecto: cada rama, de hasta dos
// sentencias, termina asignando la misma variable local (o tirando una excepción), y se puede
// escribir `x = switch (...) { case A -> ...; }`. Sin `default`, la variable tiene que tener valor
// antes del switch.
pub fn assigns_in_every_branch(switch: Node<'_>, source: &[u8]) -> bool {
    let Some(body) = switch.child_by_field_name(BODY_FIELD) else {
        return false;
    };
    let groups: Vec<Node<'_>> = named_children(body)
        .into_iter()
        .filter(|entry| !entry.kind().ends_with(COMMENT_SUFFIX))
        .collect();
    if groups.is_empty() || groups.iter().any(|group| group.kind() != GROUP) {
        return false;
    }
    let mut assigned: Option<&str> = None;
    for (index, group) in groups.iter().enumerate() {
        let is_last = index + 1 == groups.len();
        let Some(branch) = branch_target(*group, is_last, source) else {
            return false;
        };
        match (branch, assigned) {
            (Target::Skip | Target::Throws, _) => {}
            (Target::Assigns(name), None) => assigned = Some(name),
            (Target::Assigns(name), Some(previous)) if name == previous => {}
            (Target::Assigns(_), Some(_)) => return false,
        }
    }
    let Some(variable) = assigned else {
        return false;
    };
    let Some(declarator) = local_declarator(switch, variable, source) else {
        return false;
    };
    groups.iter().any(|group| has_default(*group, source))
        || assigned_before(switch, declarator, variable, source)
}

#[derive(Clone, Copy)]
enum Target<'source> {
    Skip,
    Throws,
    Assigns(&'source str),
}

fn branch_target<'source>(group: Node<'_>, is_last: bool, source: &'source [u8]) -> Option<Target<'source>> {
    let all: Vec<Node<'_>> = named_children(group)
        .into_iter()
        .filter(|child| child.kind() != LABEL && !child.kind().ends_with(COMMENT_SUFFIX))
        .collect();
    let statements = branch_statements(group);
    let ends_with_break = all.iter().any(|statement| statement.kind() == BREAK);
    let Some(last) = statements.last() else {
        let only_labels = all.is_empty() && !is_last;
        return only_labels.then_some(Target::Skip);
    };
    let falls_through = !is_last && !ends_with_break && last.kind() != THROW;
    let has_exit = statements.iter().any(|statement| {
        descendants(*statement)
            .iter()
            .any(|part| EXITS.contains(&part.kind()))
    });
    let breaks_early = statements
        .iter()
        .take(statements.len() - 1)
        .any(|statement| descendants(*statement).iter().any(|part| part.kind() == BREAK));
    if falls_through || has_exit || breaks_early || statements.len() > MAX_STATEMENTS_IN_BRANCH {
        return None;
    }
    if last.kind() == THROW {
        return Some(Target::Throws);
    }
    assigned_name(*last, source).map(Target::Assigns)
}

fn assigned_name<'source>(statement: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    if statement.kind() != EXPRESSION_STATEMENT {
        return None;
    }
    let assignment = named_children(statement).into_iter().next()?;
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let plain = assignment.kind() == ASSIGNMENT
        && text(assignment.child_by_field_name(OPERATOR_FIELD)?, source) == ASSIGN
        && target.kind() == IDENTIFIER;
    plain.then(|| text(target, source))
}

// La declaración local de la variable antes del switch, en los bloques que lo encierran dentro del
// mismo método o lambda; un parámetro o un campo no cuentan.
pub(super) fn local_declarator<'tree>(switch: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    std::iter::successors(switch.parent(), Node::parent)
        .take_while(|ancestor| !SCOPE_OWNERS.contains(&ancestor.kind()))
        .filter(|ancestor| ancestor.kind() == BLOCK)
        .find_map(|block| {
            named_children(block)
                .into_iter()
                .filter(|statement| {
                    statement.kind() == LOCAL_VARIABLE && statement.end_byte() <= switch.start_byte()
                })
                .flat_map(named_children)
                .rfind(|declarator| {
                    declarator.kind() == VARIABLE_DECLARATOR
                        && declarator
                            .child_by_field_name(NAME_FIELD)
                            .is_some_and(|declared| text(declared, source) == name)
                })
        })
}

// La variable ya tiene valor antes del switch: con inicializador (el literal de justo antes o
// cualquier otro) o asignada en una sentencia anterior del mismo bloque.
fn assigned_before(switch: Node<'_>, declarator: Node<'_>, name: &str, source: &[u8]) -> bool {
    declarator.child_by_field_name(VALUE_FIELD).is_some()
        || switch.parent().is_some_and(|block| {
            named_children(block)
                .into_iter()
                .filter(|statement| statement.end_byte() <= switch.start_byte())
                .any(|statement| assigned_name(statement, source) == Some(name))
        })
}
