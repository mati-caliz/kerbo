use tree_sitter::Node;

use super::java::{
    ANNOTATION, ARRAY_CREATION, CLASS_BODY, CONSTRUCTOR, FINAL, IDENTIFIER, METHOD, MODIFIERS, NAME_FIELD,
    OBJECT_CREATION, TYPE_FIELD, VARIABLE_DECLARATOR, has_modifier, text,
};
use super::java_chains::argument_count;
use super::java_contracts::Constraint;
use super::java_receivers::{
    Receiver, StaticImports, Variables, receiver_of, simple_type_name, variable_types,
};
use crate::syntax::{descendants, named_children, one_based};

const INVOCATION: &str = "method_invocation";
const ARGUMENTS_FIELD: &str = "arguments";
const VALUE_FIELD: &str = "value";
const KEY_FIELD: &str = "key";
const CAST: &str = "cast_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const BINARY: &str = "binary_expression";
const OPERATOR_FIELD: &str = "operator";
const CONCATENATION: &str = "+";
const NULL: &str = "null_literal";
const TRUE: &str = "true";
const FALSE: &str = "false";
const LAMBDA: &str = "lambda_expression";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const ARGUMENT_LIST: &str = "argument_list";
const ELEMENT_VALUE_PAIR: &str = "element_value_pair";
const TRY: &str = "try_statement";
const TRY_WITH_RESOURCES: &str = "try_with_resources_statement";
const CATCH: &str = "catch_clause";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const NOT_NULL_KINDS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "character_literal",
    "string_literal",
    "text_block",
    OBJECT_CREATION,
    ARRAY_CREATION,
    "class_literal",
    LAMBDA,
    "method_reference",
    "this",
];
const TEST_ANNOTATION: &str = "Test";
const EXPECTED_ELEMENTS: &[&str] = &["expected", "expectedExceptions"];
const ASSERT_THROWS: &str = "assertThrows";

// Una llamada que pasa algún `null`, `true` o `false` conocido, con lo que se sabe de cada
// argumento, para cruzarla con los contratos `fail` del método llamado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractCall {
    pub name: String,
    pub target: CallTarget,
    pub arity: usize,
    pub receiver: Receiver,
    pub arguments: Vec<Option<Constraint>>,
    pub line: usize,
    pub column: usize,
    pub exception_expected: bool,
}

// Un método traído con `import static` se busca en la clase importada si las que rodean a la
// llamada no lo declaran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallTarget {
    Constructor,
    Method,
    ImportedMethod,
}

pub fn contract_calls_in(top_level: Node<'_>, nodes: &[Node<'_>], source: &[u8]) -> Vec<ContractCall> {
    let variables = variable_types(nodes, source);
    let imports = StaticImports::of(top_level, source);
    nodes
        .iter()
        .filter_map(|node| contract_call(*node, source, &variables, &imports))
        .collect()
}

fn contract_call(
    call: Node<'_>,
    source: &[u8],
    variables: &Variables,
    imports: &StaticImports,
) -> Option<ContractCall> {
    let (name_node, target, receiver) = match call.kind() {
        INVOCATION => (
            call.child_by_field_name(NAME_FIELD)?,
            if imports.imports(call, source) {
                CallTarget::ImportedMethod
            } else {
                CallTarget::Method
            },
            receiver_of(call, source, variables),
        ),
        OBJECT_CREATION
            if !named_children(call)
                .iter()
                .any(|child| child.kind() == CLASS_BODY) =>
        {
            let created = call.child_by_field_name(TYPE_FIELD)?;
            (
                created,
                CallTarget::Constructor,
                Receiver::Type(simple_type_name(text(created, source))),
            )
        }
        _ => return None,
    };
    let arguments: Vec<Option<Constraint>> = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .map(|argument| fact(argument, call, source))
        .collect();
    if !arguments.iter().any(|argument| {
        matches!(
            argument,
            Some(Constraint::Null | Constraint::True | Constraint::False)
        )
    }) {
        return None;
    }
    let position = name_node.start_position();
    let name = if target == CallTarget::Constructor {
        simple_type_name(text(name_node, source))
    } else {
        text(name_node, source).to_owned()
    };
    Some(ContractCall {
        name,
        target,
        arity: argument_count(call),
        receiver,
        arguments,
        line: one_based(position.row),
        column: one_based(position.column),
        exception_expected: exception_expected(call, source),
    })
}

// Lo que se sabe de un argumento sin seguir el flujo: un literal, algo que nunca es nulo o una
// variable local `final` inicializada con un literal.
fn fact(argument: Node<'_>, call: Node<'_>, source: &[u8]) -> Option<Constraint> {
    match argument.kind() {
        PARENTHESIZED => named_children(argument)
            .first()
            .and_then(|inner| fact(*inner, call, source)),
        CAST => argument
            .child_by_field_name(VALUE_FIELD)
            .and_then(|inner| fact(inner, call, source)),
        NULL => Some(Constraint::Null),
        TRUE => Some(Constraint::True),
        FALSE => Some(Constraint::False),
        BINARY
            if argument
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == CONCATENATION) =>
        {
            Some(Constraint::NotNull)
        }
        IDENTIFIER => final_local_literal(argument, call, source),
        kind if NOT_NULL_KINDS.contains(&kind) => Some(Constraint::NotNull),
        _ => None,
    }
}

fn final_local_literal(identifier: Node<'_>, call: Node<'_>, source: &[u8]) -> Option<Constraint> {
    let name = text(identifier, source);
    let callable = ancestors(call).find(|node| node.kind() == METHOD || node.kind() == CONSTRUCTOR)?;
    let declarations: Vec<(Node<'_>, Node<'_>)> = descendants(callable)
        .into_iter()
        .filter(|node| node.kind() == LOCAL_VARIABLE)
        .flat_map(|declaration| {
            named_children(declaration)
                .into_iter()
                .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                .filter(|declarator| {
                    declarator
                        .child_by_field_name(NAME_FIELD)
                        .is_some_and(|declared| text(declared, source) == name)
                })
                .map(move |declarator| (declaration, declarator))
        })
        .collect();
    let [(declaration, declarator)] = declarations.as_slice() else {
        return None;
    };
    if !has_modifier(*declaration, FINAL) || declaration.end_byte() > call.start_byte() {
        return None;
    }
    match declarator.child_by_field_name(VALUE_FIELD)?.kind() {
        NULL => Some(Constraint::Null),
        TRUE => Some(Constraint::True),
        FALSE => Some(Constraint::False),
        _ => None,
    }
}

fn ancestors(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
}

// Calca `TestUtils.isExceptionExpected` de IntelliJ: dentro de un `try` con `catch`, de una lambda
// pasada a `assertThrows` o de un `@Test(expected = ...)` se espera que la llamada falle.
fn exception_expected(call: Node<'_>, source: &[u8]) -> bool {
    for ancestor in ancestors(call) {
        match ancestor.kind() {
            METHOD | CONSTRUCTOR => return expects_exception(ancestor, source),
            LAMBDA if is_assert_throws_argument(ancestor, source) => return true,
            TRY | TRY_WITH_RESOURCES
                if named_children(ancestor).iter().any(|child| child.kind() == CATCH) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn is_assert_throws_argument(lambda: Node<'_>, source: &[u8]) -> bool {
    let Some(list) = ancestors(lambda).find(|node| node.kind() != PARENTHESIZED) else {
        return false;
    };
    list.kind() == ARGUMENT_LIST
        && list.parent().is_some_and(|invocation| {
            invocation.kind() == INVOCATION
                && invocation
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| text(name, source) == ASSERT_THROWS)
        })
}

fn expects_exception(callable: Node<'_>, source: &[u8]) -> bool {
    named_children(callable)
        .into_iter()
        .filter(|child| child.kind() == MODIFIERS)
        .flat_map(named_children)
        .filter(|modifier| modifier.kind() == ANNOTATION)
        .filter(|annotation| {
            annotation
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| simple_type_name(text(name, source)) == TEST_ANNOTATION)
        })
        .filter_map(|annotation| annotation.child_by_field_name(ARGUMENTS_FIELD))
        .flat_map(named_children)
        .filter(|argument| argument.kind() == ELEMENT_VALUE_PAIR)
        .filter_map(|pair| pair.child_by_field_name(KEY_FIELD))
        .any(|key| EXPECTED_ELEMENTS.contains(&text(key, source)))
}
