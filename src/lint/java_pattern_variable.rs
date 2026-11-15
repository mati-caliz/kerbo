use tree_sitter::Node;

use super::java::{IDENTIFIER, NAME_FIELD, TYPE_FIELD, VARIABLE_DECLARATOR, text, unparenthesized};
use super::java_fall_through::cannot_complete_normally;
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const PATTERNS_RELEASE: u32 = 16;
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const CAST: &str = "cast_expression";
const INSTANCEOF: &str = "instanceof_expression";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const ASSIGNMENT: &str = "assignment_expression";
const UPDATE: &str = "update_expression";
const BLOCK: &str = "block";
const SWITCH_GROUP: &str = "switch_block_statement_group";
const SWITCH_LABEL: &str = "switch_label";
const IF: &str = "if_statement";
const WHILE: &str = "while_statement";
const FOR: &str = "for_statement";
const DO: &str = "do_statement";
const BREAK: &str = "break_statement";
const RECORD_PATTERN: &str = "record_pattern";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const OPERAND_FIELD: &str = "operand";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const PATTERN_FIELD: &str = "pattern";
const COMMENT_SUFFIX: &str = "comment";
const AND: &str = "&&";
const OR: &str = "||";
const NOT: &str = "!";
const INFERRED: &str = "var";
const TYPE_ARGUMENTS_START: char = '<';
const TYPE_ARGUMENTS_END: char = '>';
const ARGUMENT_SEPARATOR: char = ',';
const WILDCARD: &str = "?";
pub(super) const PRIMITIVES: [&str; 8] = [
    "boolean", "byte", "char", "short", "int", "long", "float", "double",
];
const BLOCK_STATEMENT_OWNERS: [&str; 7] = [
    IF,
    WHILE,
    FOR,
    DO,
    "enhanced_for_statement",
    BLOCK,
    "labeled_statement",
];

// `PatternVariableCanBeUsed` de IntelliJ, en la forma que reporta en lote: una variable local que
// se inicializa con un cast (`Foo foo = (Foo) objeto;`) donde un `objeto instanceof Foo` anterior ya
// lo garantiza, y alcanza con `objeto instanceof Foo foo`. El instanceof se busca como IntelliJ: en
// la condición del if o del bucle que encierra la declaración, o en un if anterior del mismo bloque
// que sale cuando no se cumple; si la variable del cast se reasigna en el medio, no vale.
pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if release.is_some_and(|release| release < PATTERNS_RELEASE) {
        return Vec::new();
    }
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == LOCAL_VARIABLE)
        .flat_map(|declaration| {
            named_children(declaration)
                .into_iter()
                .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                .filter_map(move |declarator| replaceable(declaration, declarator, source))
        })
        .collect()
}

fn replaceable(declaration: Node<'_>, declarator: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let name = declarator.child_by_field_name(NAME_FIELD)?;
    let cast = unparenthesized(declarator.child_by_field_name(VALUE_FIELD)?);
    if cast.kind() != CAST {
        return None;
    }
    let cast_type = text(cast.child_by_field_name(TYPE_FIELD)?, source).trim();
    let declared = text(declaration.child_by_field_name(TYPE_FIELD)?, source).trim();
    let operand = cast.child_by_field_name(VALUE_FIELD)?;
    let is_downcast = (declared == cast_type || declared == INFERRED)
        && !PRIMITIVES.contains(&cast_type)
        && !is_unchecked(cast_type)
        && !is_redundant(operand, cast_type, source);
    if !is_downcast {
        return None;
    }
    let candidate = Candidate {
        operand: compact(unparenthesized(operand), source),
        operand_variable: (unparenthesized(operand).kind() == IDENTIFIER)
            .then(|| text(unparenthesized(operand), source).to_owned()),
        cast_type: cast_type.to_owned(),
    };
    let instance_of = corresponding_instance_of(declaration, &candidate, source)?;
    let pattern = instance_of.child_by_field_name(PATTERN_FIELD);
    if pattern.is_some_and(|pattern| pattern.kind() == RECORD_PATTERN) {
        return None;
    }
    Some(finding(
        LintKind::Java(JavaRule::PatternVariableCanBeUsed),
        name,
        text(name, source),
        None,
    ))
}

pub(super) struct Candidate {
    pub(super) operand: String,
    pub(super) operand_variable: Option<String>,
    pub(super) cast_type: String,
}

pub(super) fn is_unchecked(cast_type: &str) -> bool {
    cast_type
        .split_once(TYPE_ARGUMENTS_START)
        .and_then(|(_, arguments)| arguments.strip_suffix(TYPE_ARGUMENTS_END))
        .is_some_and(|arguments| {
            arguments
                .split(ARGUMENT_SEPARATOR)
                .any(|argument| argument.trim() != WILDCARD)
        })
}

pub(super) fn is_redundant(operand: Node<'_>, cast_type: &str, source: &[u8]) -> bool {
    let operand = unparenthesized(operand);
    operand.kind() == IDENTIFIER
        && declared_type(operand, text(operand, source), source)
            .is_some_and(|written| text(written, source).trim() == cast_type)
}

pub(super) fn compact(node: Node<'_>, source: &[u8]) -> String {
    text(node, source)
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

// `walkBackAndUpToFindInstanceOf` de IntelliJ: desde la sentencia, los if y bucles anteriores del
// mismo bloque, y después la condición del if o el bucle que la encierra, subiendo hasta arriba.
pub(super) fn corresponding_instance_of<'tree>(
    statement: Node<'tree>,
    candidate: &Candidate,
    source: &[u8],
) -> Option<Node<'tree>> {
    let mut context = statement;
    loop {
        let parent = context.parent()?;
        if parent.kind() == BLOCK || parent.kind() == SWITCH_GROUP {
            match previous_statements(context, candidate, source) {
                Scan::Found(instance_of) => return Some(instance_of),
                Scan::Reassigned => return None,
                Scan::Nothing => {}
            }
            if parent.kind() == BLOCK
                && parent
                    .parent()
                    .is_some_and(|owner| BLOCK_STATEMENT_OWNERS.contains(&owner.kind()))
            {
                context = parent;
            }
        }
        let owner = context.parent()?;
        if let Some(found) = from_enclosing(context, owner, candidate, source) {
            return Some(found);
        }
        context = owner;
    }
}

enum Scan<'tree> {
    Found(Node<'tree>),
    Reassigned,
    Nothing,
}

fn previous_statements<'tree>(context: Node<'tree>, candidate: &Candidate, source: &[u8]) -> Scan<'tree> {
    let mut previous = context.prev_named_sibling();
    while let Some(statement) = previous {
        previous = statement.prev_named_sibling();
        if statement.kind().ends_with(COMMENT_SUFFIX) {
            continue;
        }
        if let Some(found) = from_previous(statement, candidate, source) {
            return Scan::Found(found);
        }
        if statement.kind() == SWITCH_LABEL {
            break;
        }
        if candidate
            .operand_variable
            .as_deref()
            .is_some_and(|variable| assigns(statement, variable, source))
        {
            return Scan::Reassigned;
        }
    }
    Scan::Nothing
}

fn from_previous<'tree>(statement: Node<'tree>, candidate: &Candidate, source: &[u8]) -> Option<Node<'tree>> {
    let condition = statement.child_by_field_name(CONDITION_FIELD)?;
    if !mentions_operand(condition, candidate, source) {
        return None;
    }
    match statement.kind() {
        IF => {
            let then_completes = statement
                .child_by_field_name(CONSEQUENCE_FIELD)
                .is_none_or(|branch| !cannot_complete_normally(branch, source));
            let else_completes = statement
                .child_by_field_name(ALTERNATIVE_FIELD)
                .is_none_or(|branch| !cannot_complete_normally(branch, source));
            (then_completes != else_completes)
                .then(|| instance_of_in(condition, candidate, then_completes, source))
                .flatten()
        }
        WHILE | DO | FOR if !breaks_out(statement) => instance_of_in(condition, candidate, false, source),
        _ => None,
    }
}

fn from_enclosing<'tree>(
    context: Node<'tree>,
    parent: Node<'tree>,
    candidate: &Candidate,
    source: &[u8],
) -> Option<Node<'tree>> {
    let condition = parent.child_by_field_name(CONDITION_FIELD)?;
    let is_field = |field: &str| {
        parent
            .child_by_field_name(field)
            .is_some_and(|branch| branch.id() == context.id())
    };
    match parent.kind() {
        IF if is_field(CONSEQUENCE_FIELD) => instance_of_in(condition, candidate, true, source),
        IF if is_field(ALTERNATIVE_FIELD) => instance_of_in(condition, candidate, false, source),
        WHILE | FOR if condition.id() != context.id() => instance_of_in(condition, candidate, true, source),
        _ => None,
    }
}

fn mentions_operand(condition: Node<'_>, candidate: &Candidate, source: &[u8]) -> bool {
    descendants(condition).into_iter().any(|node| {
        node.kind() == INSTANCEOF
            && node
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|operand| compact(unparenthesized(operand), source) == candidate.operand)
    })
}

fn breaks_out(statement: Node<'_>) -> bool {
    descendants(statement)
        .into_iter()
        .any(|node| node.kind() == BREAK)
}

// `findInstanceOf` de IntelliJ: atraviesa paréntesis, negaciones y `&&` (u `||` cuando se busca el
// caso falso) hasta un instanceof del mismo operando con el tipo del cast.
pub(super) fn instance_of_in<'tree>(
    condition: Node<'tree>,
    candidate: &Candidate,
    when_true: bool,
    source: &[u8],
) -> Option<Node<'tree>> {
    match condition.kind() {
        PARENTHESIZED => instance_of_in(
            named_children(condition).into_iter().next()?,
            candidate,
            when_true,
            source,
        ),
        UNARY if text(condition.child_by_field_name(OPERATOR_FIELD)?, source) == NOT => instance_of_in(
            condition.child_by_field_name(OPERAND_FIELD)?,
            candidate,
            !when_true,
            source,
        ),
        BINARY => {
            let operator = text(condition.child_by_field_name(OPERATOR_FIELD)?, source);
            if !(operator == AND && when_true || operator == OR && !when_true) {
                return None;
            }
            [LEFT_FIELD, RIGHT_FIELD]
                .iter()
                .filter_map(|field| condition.child_by_field_name(field))
                .find_map(|operand| instance_of_in(operand, candidate, when_true, source))
        }
        INSTANCEOF if when_true => {
            let operand = condition.child_by_field_name(LEFT_FIELD)?;
            let checked = condition.child_by_field_name(RIGHT_FIELD)?;
            let checked_type = text(checked, source).trim();
            let raw_cast = candidate
                .cast_type
                .split(TYPE_ARGUMENTS_START)
                .next()
                .unwrap_or_default();
            let compatible = checked_type == candidate.cast_type || checked_type == raw_cast;
            (compact(unparenthesized(operand), source) == candidate.operand && compatible)
                .then_some(condition)
        }
        _ => None,
    }
}

fn assigns(statement: Node<'_>, variable: &str, source: &[u8]) -> bool {
    descendants(statement).into_iter().any(|node| {
        let target = match node.kind() {
            ASSIGNMENT => node.child_by_field_name(LEFT_FIELD),
            UPDATE => named_children(node).into_iter().next(),
            _ => None,
        };
        target.is_some_and(|target| target.kind() == IDENTIFIER && text(target, source) == variable)
    })
}
