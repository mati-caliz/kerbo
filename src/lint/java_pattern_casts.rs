use tree_sitter::Node;

use super::java::{IDENTIFIER, NAME_FIELD, TYPE_FIELD, VARIABLE_DECLARATOR, text};
use super::java_pattern_variable::{
    Candidate, PRIMITIVES, compact, corresponding_instance_of, instance_of_in, is_redundant, is_unchecked,
};
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::descendants;

const PATTERNS_RELEASE: u32 = 16;
const CAST: &str = "cast_expression";
const FIELD_ACCESS: &str = "field_access";
const BINARY: &str = "binary_expression";
const TERNARY: &str = "ternary_expression";
const LAMBDA: &str = "lambda_expression";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const ASSIGNMENT: &str = "assignment_expression";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const STATEMENT_SUFFIX: &str = "_statement";
const AND: &str = "&&";
const OR: &str = "||";
const TYPE_ARGUMENTS_START: char = '<';
const STOPS_THE_WALK: [&str; 4] = [
    "block",
    "class_body",
    "method_declaration",
    "constructor_declaration",
];

// La otra forma de `PatternVariableCanBeUsed`: un cast suelto (`((Foo) objeto).bar()`) donde el
// instanceof que lo garantiza ya declara una variable de patrón (`objeto instanceof Foo foo`), que
// se puede usar en su lugar. Se marca el tipo del cast.
pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if release.is_some_and(|release| release < PATTERNS_RELEASE) {
        return Vec::new();
    }
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == CAST)
        .filter_map(|cast| {
            let cast_type = cast.child_by_field_name(TYPE_FIELD)?;
            let pattern_variable = existing_pattern_variable(cast, cast_type, source)?;
            Some(finding(
                LintKind::Java(JavaRule::PatternVariableCanBeUsed),
                cast_type,
                &pattern_variable,
                None,
            ))
        })
        .collect()
}

fn existing_pattern_variable(cast: Node<'_>, cast_type: Node<'_>, source: &[u8]) -> Option<String> {
    if cast
        .parent()
        .is_some_and(|parent| parent.kind() == VARIABLE_DECLARATOR)
    {
        return None;
    }
    let operand = cast.child_by_field_name(VALUE_FIELD)?;
    let written_type = text(cast_type, source).trim();
    let is_reference = operand.kind() == IDENTIFIER || operand.kind() == FIELD_ACCESS;
    let checked = !is_unchecked(written_type) || has_type_arguments(operand, source);
    if !is_reference
        || PRIMITIVES.contains(&written_type)
        || !checked
        || is_redundant(operand, written_type, source)
    {
        return None;
    }
    let candidate = Candidate {
        operand: compact(operand, source),
        operand_variable: (operand.kind() == IDENTIFIER).then(|| text(operand, source).to_owned()),
        cast_type: written_type.to_owned(),
    };
    let instance_of = instance_of_around(cast, &candidate, source)?;
    let variable = instance_of.child_by_field_name(NAME_FIELD)?;
    let name = text(variable, source);
    (!is_reassigned(instance_of, name, source)).then(|| name.to_owned())
}

fn has_type_arguments(operand: Node<'_>, source: &[u8]) -> bool {
    operand.kind() == IDENTIFIER
        && declared_type(operand, text(operand, source), source)
            .is_some_and(|written| text(written, source).contains(TYPE_ARGUMENTS_START))
}

// `findCorrespondingInstanceOf` de IntelliJ sin variable: primero dentro de la expresión (el lado
// izquierdo de un `&&` o `||`, la condición de un ternario), y al llegar a una sentencia, como para
// una variable local.
fn instance_of_around<'tree>(cast: Node<'tree>, candidate: &Candidate, source: &[u8]) -> Option<Node<'tree>> {
    let mut child = cast;
    let mut context = cast.parent()?;
    loop {
        if let Some(found) = within_expression(context, child, candidate, source) {
            return Some(found);
        }
        match context.kind() {
            LAMBDA => return None,
            kind if kind.ends_with(STATEMENT_SUFFIX) || kind == LOCAL_VARIABLE => {
                return corresponding_instance_of(context, candidate, source);
            }
            kind if STOPS_THE_WALK.contains(&kind) => return None,
            _ => {}
        }
        child = context;
        context = context.parent()?;
    }
}

fn within_expression<'tree>(
    context: Node<'tree>,
    child: Node<'tree>,
    candidate: &Candidate,
    source: &[u8],
) -> Option<Node<'tree>> {
    match context.kind() {
        BINARY => {
            let operator = text(context.child_by_field_name(OPERATOR_FIELD)?, source);
            let right = context.child_by_field_name(RIGHT_FIELD)?;
            if !(operator == AND || operator == OR) || right.id() != child.id() {
                return None;
            }
            instance_of_in(
                context.child_by_field_name(LEFT_FIELD)?,
                candidate,
                operator == AND,
                source,
            )
        }
        TERNARY => {
            let condition = context.child_by_field_name(CONDITION_FIELD)?;
            if condition.id() == child.id() {
                return None;
            }
            let when_true = context
                .child_by_field_name(CONSEQUENCE_FIELD)
                .is_some_and(|branch| branch.id() == child.id());
            instance_of_in(condition, candidate, when_true, source)
        }
        _ => None,
    }
}

fn is_reassigned(instance_of: Node<'_>, name: &str, source: &[u8]) -> bool {
    let scope = std::iter::successors(instance_of.parent(), Node::parent)
        .find(|ancestor| STOPS_THE_WALK.contains(&ancestor.kind()))
        .unwrap_or(instance_of);
    descendants(scope).into_iter().any(|node| {
        node.kind() == ASSIGNMENT
            && node
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|target| target.kind() == IDENTIFIER && text(target, source) == name)
    })
}
