use tree_sitter::Node;

use super::nodes::{
    ARGUMENTS_FIELD, BINARY, CALL, FUNCTION_FIELD, LEFT_FIELD, MEMBER, NAME_FIELD, OBJECT_FIELD,
    PARENTHESIZED, RIGHT_FIELD, SUBSCRIPT, VALUE_FIELD, VARIABLE_DECLARATOR, first_named_child, operator,
    without_parentheses,
};
use super::tokens::{AND, OR};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const OPTIONAL_CHAIN_FIELD: &str = "optional_chain";
const PATTERNS: &[&str] = &["object_pattern", "array_pattern"];
const ASSIGNMENT: &str = "assignment_expression";
const ASSIGNMENT_PATTERN: &str = "assignment_pattern";
const NEW: &str = "new_expression";
const CONSTRUCTOR_FIELD: &str = "constructor";
const SPREAD: &str = "spread_element";
const OBJECT: &str = "object";
const FOR_IN: &str = "for_in_statement";
const OF: &str = "of";
const EXTENDS_CLAUSE: &str = "extends_clause";
const CLASS_HERITAGE: &str = "class_heritage";
const HERITAGE_CLAUSES: &[&str] = &[EXTENDS_CLAUSE, "implements_clause"];
const YIELD: &str = "yield_expression";
const DELEGATE: &str = "*";
const WITH: &str = "with_statement";
const UNSAFE_RELATIONS: &[&str] = &["in", "instanceof"];
const NULLISH: &str = "??";
const SEQUENCE: &str = "sequence_expression";
const TERNARY: &str = "ternary_expression";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const AWAIT: &str = "await_expression";
const NON_NULL: &str = "non_null_expression";
const TEMPLATE_STRING: &str = "template_string";
const CHAIN_LINKS: &[&str] = &[MEMBER, SUBSCRIPT, CALL];

// Si el nodo es, en ESTree, un `ChainExpression`: un acceso o una llamada con `?.` en su cadena.
pub fn is_chain(node: Node<'_>) -> bool {
    let mut current = Some(node);
    while let Some(link) = current.filter(|link| CHAIN_LINKS.contains(&link.kind())) {
        if link.child_by_field_name(OPTIONAL_CHAIN_FIELD).is_some() {
            return true;
        }
        let field = if link.kind() == CALL {
            FUNCTION_FIELD
        } else {
            OBJECT_FIELD
        };
        current = link.child_by_field_name(field);
    }
    false
}

// `no-unsafe-optional-chaining`: un `?.` que puede dar `undefined` donde eso tira un TypeError.
pub fn unsafe_optional_chains(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mut found = Vec::new();
    for node in &context.nodes {
        for checked in unsafe_positions(*node, context) {
            undefined_short_circuits(checked, &mut found, context);
        }
    }
    found
        .into_iter()
        .map(|chain| report(EslintRule::NoUnsafeOptionalChaining, chain, ""))
        .collect()
}

fn unsafe_positions<'tree>(node: Node<'tree>, context: &Context<'tree, '_>) -> Vec<Node<'tree>> {
    let field = |name: &str| node.child_by_field_name(name);
    let checked = match node.kind() {
        ASSIGNMENT | ASSIGNMENT_PATTERN
            if field(LEFT_FIELD).is_some_and(|left| PATTERNS.contains(&left.kind())) =>
        {
            field(RIGHT_FIELD)
        }
        VARIABLE_DECLARATOR if field(NAME_FIELD).is_some_and(|name| PATTERNS.contains(&name.kind())) => {
            field(VALUE_FIELD)
        }
        CALL if field(OPTIONAL_CHAIN_FIELD).is_none() => {
            let callee = field(FUNCTION_FIELD).filter(|callee| callee.kind() == PARENTHESIZED);
            if field(ARGUMENTS_FIELD).is_some_and(|arguments| arguments.kind() == TEMPLATE_STRING) {
                field(FUNCTION_FIELD)
            } else {
                callee
            }
        }
        MEMBER | SUBSCRIPT if field(OPTIONAL_CHAIN_FIELD).is_none() => {
            field(OBJECT_FIELD).filter(|object| object.kind() == PARENTHESIZED)
        }
        NEW => field(CONSTRUCTOR_FIELD),
        SPREAD if node.parent().is_some_and(|parent| parent.kind() != OBJECT) => first_named_child(node),
        FOR_IN if has_token(node, OF) => field(RIGHT_FIELD),
        EXTENDS_CLAUSE => field(VALUE_FIELD).or_else(|| first_named_child(node)),
        CLASS_HERITAGE => first_named_child(node).filter(|value| !HERITAGE_CLAUSES.contains(&value.kind())),
        YIELD if has_token(node, DELEGATE) => first_named_child(node),
        BINARY if UNSAFE_RELATIONS.contains(&operator(node, context.source)) => field(RIGHT_FIELD),
        WITH => field(OBJECT_FIELD),
        _ => None,
    };
    checked.into_iter().collect()
}

fn has_token(node: Node<'_>, token: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| !child.is_named() && child.kind() == token)
}

// `checkUndefinedShortCircuit`: baja por lo que puede terminar valiendo la cadena opcional.
fn undefined_short_circuits<'tree>(
    node: Node<'tree>,
    found: &mut Vec<Node<'tree>>,
    context: &Context<'tree, '_>,
) {
    let node = without_parentheses(node);
    if is_chain(node) {
        found.push(node);
        return;
    }
    let field = |name: &str| node.child_by_field_name(name);
    match node.kind() {
        BINARY => match operator(node, context.source) {
            OR | NULLISH => {
                if let Some(right) = field(RIGHT_FIELD) {
                    undefined_short_circuits(right, found, context);
                }
            }
            AND => {
                for side in [field(LEFT_FIELD), field(RIGHT_FIELD)].into_iter().flatten() {
                    undefined_short_circuits(side, found, context);
                }
            }
            _ => {}
        },
        SEQUENCE => {
            let mut cursor = node.walk();
            if let Some(last) = node.named_children(&mut cursor).last() {
                undefined_short_circuits(last, found, context);
            }
        }
        TERNARY => {
            for branch in [field(CONSEQUENCE_FIELD), field(ALTERNATIVE_FIELD)]
                .into_iter()
                .flatten()
            {
                undefined_short_circuits(branch, found, context);
            }
        }
        AWAIT => {
            if let Some(argument) = first_named_child(node) {
                undefined_short_circuits(argument, found, context);
            }
        }
        _ => {}
    }
}

// `no-non-null-asserted-optional-chain`: `a?.b!`, que afirma que no es nulo justo lo que puede serlo.
pub fn non_null_optional_chains(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[NON_NULL])
        .filter_map(|assertion| {
            let operand = first_named_child(assertion)?;
            let chain = without_parentheses(operand);
            let ends_chain = operand.kind() == PARENTHESIZED || !continues_chain(assertion);
            (is_chain(chain) && ends_chain).then_some(chain)
        })
        .map(|chain| report(EslintRule::NoNonNullAssertedOptionalChain, chain, ""))
        .collect()
}

// Un `a?.b!.c` sigue siendo una sola cadena: el `!` queda adentro y ESLint no lo marca.
fn continues_chain(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        let field = if parent.kind() == CALL {
            FUNCTION_FIELD
        } else {
            OBJECT_FIELD
        };
        CHAIN_LINKS.contains(&parent.kind())
            && parent
                .child_by_field_name(field)
                .is_some_and(|link| link.id() == node.id())
    })
}
