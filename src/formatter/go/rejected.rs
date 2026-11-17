use tree_sitter::Node;

use super::convert::{children, named};
use crate::syntax::descendants;

const IMPORT: &str = "import_declaration";
const PACKAGE: &str = "package_clause";
const PARAMETERS: &str = "parameter_list";
const NAME_FIELD: &str = "name";
const TYPE_FIELD: &str = "type";
const PARAMETER: &str = "parameter_declaration";
const TYPE_IDENTIFIER: &str = "type_identifier";
const RANGE: &str = "range_clause";
const LEFT_FIELD: &str = "left";
const MOST_RANGE_VARIABLES: usize = 2;
const LITERAL_VALUE: &str = "literal_value";
const CLOSING_BRACE: &str = "}";
const LIST_CLOSERS: &[&str] = &[")", "]"];
// Los paréntesis que agrupan declaraciones: ahí cada renglón termina en punto y coma.
const GROUPED_DECLARATIONS: &[&str] = &[
    "import_spec_list",
    "const_declaration",
    "var_spec_list",
    "type_declaration",
];
// Los símbolos después de los que Go pone un punto y coma si termina el renglón; también lo pone
// después de un identificador o un literal, que en el árbol son hojas con nombre.
const ENDS_A_STATEMENT: &[&str] = &[
    ")",
    "]",
    "}",
    "\"",
    "`",
    "break",
    "continue",
    "fallthrough",
    "return",
    "++",
    "--",
];

// «imports must appear before other declarations».
fn late_import(root: Node<'_>) -> Option<Node<'_>> {
    named(root)
        .into_iter()
        .skip_while(|node| node.kind() == PACKAGE || node.kind() == IMPORT)
        .find(|node| node.kind() == IMPORT)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Parameter {
    NameAndType,
    LoneIdentifier,
    LoneType,
}

fn parameter(declaration: Node<'_>) -> Parameter {
    if declaration.child_by_field_name(NAME_FIELD).is_some() {
        return Parameter::NameAndType;
    }
    let is_an_identifier = declaration
        .child_by_field_name(TYPE_FIELD)
        .is_some_and(|typ| typ.kind() == TYPE_IDENTIFIER);
    if is_an_identifier && declaration.kind() == PARAMETER {
        Parameter::LoneIdentifier
    } else {
        Parameter::LoneType
    }
}

// «missing parameter name» y «missing parameter type»: si un parámetro tiene nombre, todos lo
// tienen. tree-sitter-go parte las listas largas de nombres (`a, b, c int`) en identificadores
// sueltos seguidos del último con su tipo; go/parser los lee igual y después le da a cada suelto el
// tipo que tiene a la derecha: sólo es un error el suelto que no tiene ninguno, o un tipo sin nombre.
fn mixes_named_and_unnamed_parameters(node: Node<'_>) -> bool {
    if node.kind() != PARAMETERS {
        return false;
    }
    let parameters: Vec<Parameter> = named(node).into_iter().map(parameter).collect();
    let Some(last_with_type) = parameters
        .iter()
        .rposition(|parameter| *parameter == Parameter::NameAndType)
    else {
        return false;
    };
    parameters.contains(&Parameter::LoneType)
        || parameters
            .iter()
            .skip(last_with_type)
            .any(|parameter| *parameter == Parameter::LoneIdentifier)
}

// «expected at most 2 expressions».
fn ranges_over_too_many_variables(node: Node<'_>) -> bool {
    node.kind() == RANGE
        && node
            .child_by_field_name(LEFT_FIELD)
            .is_some_and(|left| named(left).len() > MOST_RANGE_VARIABLES)
}

fn closes_a_comma_list(token: Node<'_>) -> bool {
    let Some(parent) = token.parent() else {
        return false;
    };
    if token.is_named() {
        return false;
    }
    if token.kind() == CLOSING_BRACE {
        return parent.kind() == LITERAL_VALUE;
    }
    LIST_CLOSERS.contains(&token.kind()) && !GROUPED_DECLARATIONS.contains(&parent.kind())
}

fn last_leaf_before(token: Node<'_>) -> Option<Node<'_>> {
    let siblings = children(token.parent()?);
    let position = siblings.iter().position(|sibling| *sibling == token)?;
    let mut leaf = *siblings.get(position.checked_sub(1)?)?;
    while let Some(last) = children(leaf).last() {
        leaf = *last;
    }
    Some(leaf)
}

// «missing ',' before newline»: para tree-sitter el salto de línea es un espacio más, pero Go pone
// ahí un punto y coma y la lista (argumentos, parámetros, un literal, un índice o un paréntesis)
// queda sin cerrar. Devuelve lo último que hay antes del salto.
fn element_before_a_newline_without_comma(closer: Node<'_>) -> Option<Node<'_>> {
    if !closes_a_comma_list(closer) {
        return None;
    }
    let last = last_leaf_before(closer)?;
    let ends_a_statement = last.is_named() || ENDS_A_STATEMENT.contains(&last.kind());
    (ends_a_statement && last.end_position().row < closer.start_position().row).then_some(last)
}

// Lo que tree-sitter-go acepta y go/parser rechaza con un error de sintaxis: gofmt deja ese archivo
// como está.
pub fn first_rejected(root: Node<'_>) -> Option<Node<'_>> {
    late_import(root).or_else(|| {
        descendants(root).into_iter().find_map(|node| {
            if mixes_named_and_unnamed_parameters(node) || ranges_over_too_many_variables(node) {
                return Some(node);
            }
            element_before_a_newline_without_comma(node)
        })
    })
}
