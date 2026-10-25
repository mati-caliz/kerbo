use tree_sitter::Node;

use super::model::Model;
use super::qualified::qualified_name;
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    BODY_FIELD, CALL, CLASS, COMMENT, DECORATED, EXPRESSION_STATEMENT, FUNCTION, FUNCTION_FIELD, NONE,
    STRING, is_string, named, string_prefix, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::fix_edits::commented_unsafe;
use crate::lint::python::statement_removal::{delete_statement, isolation_group};
use crate::rules::PythonRule;

const RETURN: &str = "return_statement";
const RETURN_KEYWORD: &str = "return";
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];
const YIELD: &str = "yield";
const TYPE_NODE: &str = "type";
const DECORATOR: &str = "decorator";
const PARAMETERS_FIELD: &str = "parameters";
const RETURN_TYPE_FIELD: &str = "return_type";
const PROPERTIES: [&str; 5] = [
    "builtins.property",
    "enum.property",
    "functools.cached_property",
    "abc.abstractproperty",
    "types.DynamicClassAttribute",
];

// Los `return` de una función, sin los de funciones y clases anidadas, y si es un generador.
#[derive(Default)]
struct Returns<'tree> {
    statements: Vec<Node<'tree>>,
    is_generator: bool,
}

// RET501: `return None` en una función que nunca devuelve otra cosa, donde alcanza con `return`.
// Como ruff, no mira funciones de una sola sentencia `return`, generadores, propiedades ni las que
// anotan un tipo de retorno distinto de `None`. `decorators` se resuelven en el alcance de la `def`.
pub fn unnecessary_return_none(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    decorators: &[Node<'_>],
) -> Vec<Report> {
    let Some(body) = function.child_by_field_name(BODY_FIELD) else {
        return Vec::new();
    };
    let statements = without_comments(body);
    if matches!(statements.as_slice(), [] | [_])
        && statements.iter().all(|statement| statement.kind() == RETURN)
    {
        return Vec::new();
    }
    let mut returns = Returns::default();
    for statement in statements {
        collect(statement, &mut returns);
    }
    let annotated_otherwise = function
        .child_by_field_name(RETURN_TYPE_FIELD)
        .is_some_and(|annotation| !is_none(inner_type(annotation)));
    let has_result = returns
        .statements
        .iter()
        .any(|statement| returned_value(*statement).is_some_and(|value| !is_none(value)));
    if returns.is_generator || has_result || annotated_otherwise || is_property(model, source, decorators) {
        return Vec::new();
    }
    returns
        .statements
        .into_iter()
        .filter(|statement| returned_value(*statement).is_some_and(is_none))
        .map(|statement| {
            let fix = commented_unsafe(Fix::replace(statement, RETURN_KEYWORD, source), statement, source);
            Report::at(PythonRule::RET501, statement, "").with_fix(Some(fix))
        })
        .collect()
}

// PLR1711: un `return` a secas (o `return None`) al final de una función que no tiene otro
// `return`, salvo que sea lo único que tiene (con o sin docstring).
pub fn useless_return(source: &[u8], function: Node<'_>) -> Option<Report> {
    let annotated_otherwise = function
        .child_by_field_name(RETURN_TYPE_FIELD)
        .is_some_and(|annotation| !is_none(inner_type(annotation)));
    let statements = without_comments(function.child_by_field_name(BODY_FIELD)?);
    let last = statements.last().copied().filter(|last| last.kind() == RETURN)?;
    let only_docstring_before = match statements.as_slice() {
        [_] => true,
        [first, _] => is_docstring(*first, source),
        _ => false,
    };
    if annotated_otherwise
        || only_docstring_before
        || returned_value(last).is_some_and(|value| !is_none(value))
    {
        return None;
    }
    let mut returns = Returns::default();
    for statement in statements {
        collect(statement, &mut returns);
    }
    (returns.statements.len() == 1).then(|| {
        let fix = commented_unsafe(useless_return_fix(last, source), function, source);
        Report::at(PythonRule::PLR1711, last, "").with_fix(Some(fix))
    })
}

// PLR1711 respeta un comentario al lado del `return`; si no, lo borra como `delete_stmt` (un `pass`
// si queda solo).
fn useless_return_fix(statement: Node<'_>, source: &[u8]) -> Fix {
    let comment = statement
        .next_sibling()
        .filter(|next| next.kind() == COMMENT && next.start_position().row == statement.end_position().row);
    comment.map_or_else(
        || delete_statement(statement, source),
        |comment| {
            Fix::replace_range(statement.start_position(), comment.start_position(), "", source)
                .isolated(isolation_group(statement))
        },
    )
}

fn is_docstring(statement: Node<'_>, source: &[u8]) -> bool {
    statement.kind() == EXPRESSION_STATEMENT
        && matches!(without_comments(statement).as_slice(), [value] if is_plain_string(*value, source))
}

fn is_plain_string(value: Node<'_>, source: &[u8]) -> bool {
    let parts = if value.kind() == STRING {
        vec![value]
    } else {
        named(value)
    };
    is_string(value)
        && text_value(value, source).is_some()
        && !parts
            .iter()
            .any(|part| string_prefix(*part, source).contains(TEMPLATE_PREFIXES))
}

fn without_comments(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// El `ReturnVisitor` de ruff: no entra al cuerpo de una `def` ni de una `class` anidada, pero sí a
// sus decoradores, parámetros y anotación de retorno (un `yield` ahí también la vuelve generador).
fn collect<'tree>(node: Node<'tree>, returns: &mut Returns<'tree>) {
    match node.kind() {
        RETURN => {
            returns.statements.push(node);
            for child in named(node) {
                collect(child, returns);
            }
        }
        YIELD => returns.is_generator = true,
        FUNCTION => [PARAMETERS_FIELD, RETURN_TYPE_FIELD]
            .iter()
            .filter_map(|field| node.child_by_field_name(field))
            .for_each(|child| collect(child, returns)),
        CLASS => {}
        DECORATED => named(node)
            .into_iter()
            .filter(|child| child.kind() == DECORATOR || child.kind() == FUNCTION)
            .for_each(|child| collect(child, returns)),
        _ => named(node).into_iter().for_each(|child| collect(child, returns)),
    }
}

fn returned_value(statement: Node<'_>) -> Option<Node<'_>> {
    without_comments(statement).first().copied().map(unparenthesized)
}

fn is_none(value: Node<'_>) -> bool {
    unparenthesized(value).kind() == NONE
}

fn inner_type(annotation: Node<'_>) -> Node<'_> {
    if annotation.kind() == TYPE_NODE {
        named(annotation).first().copied().unwrap_or(annotation)
    } else {
        annotation
    }
}

// `is_property`: un decorador (o la llamada a uno) que es `property` o una de sus variantes.
fn is_property(model: &Model<'_>, source: &[u8], decorators: &[Node<'_>]) -> bool {
    decorators.iter().any(|decorator| {
        let Some(expression) = without_comments(*decorator).first().copied().map(unparenthesized) else {
            return false;
        };
        let callable = if expression.kind() == CALL {
            expression
                .child_by_field_name(FUNCTION_FIELD)
                .unwrap_or(expression)
        } else {
            expression
        };
        qualified_name(model, callable, source).is_some_and(|name| PROPERTIES.contains(&name.as_str()))
    })
}
