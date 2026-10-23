use tree_sitter::Node;

use crate::syntax::{descendants, named_children};

const COMPOSITE: &str = "composite_literal";
const UNARY: &str = "unary_expression";
const CHANNEL: &str = "channel_type";
const ARROW: &str = "<-";
const CHAN: &str = "chan";
const CONVERSION: &str = "type_conversion_expression";
const POINTER: &str = "pointer_type";
const POSTFIX_OPERANDS: &[(&str, &str)] = &[
    ("selector_expression", "operand"),
    ("call_expression", "function"),
    ("index_expression", "operand"),
    ("slice_expression", "operand"),
    ("type_assertion_expression", "operand"),
];
const TYPE_FIELD: &str = "type";
const NAMED_TYPES: &[&str] = &["type_identifier", "qualified_type", "generic_type"];
// Los campos de cada sentencia que van antes de su llave: ahí la llave abre el cuerpo. En un `for`,
// todo lo que no es el cuerpo.
const HEADER_FIELDS: &[(&str, &[&str])] = &[
    ("if_statement", &["initializer", "condition"]),
    ("expression_switch_statement", &["initializer", "value"]),
    ("type_switch_statement", &["initializer", "value", "alias"]),
];
const HEADER_CLAUSES: &[&str] = &["for_clause", "range_clause"];
const FOR: &str = "for_statement";
const BODY_FIELD: &str = "body";
// Lo que encierra un literal entre delimitadores propios: ahí Go sí lo lee como literal.
const ENCLOSING: &[&str] = &[
    "parenthesized_expression",
    "argument_list",
    "index_expression",
    "slice_expression",
    "literal_value",
    "func_literal",
    "block",
    "expression_case",
    "type_case",
    "communication_case",
    "default_case",
];

// En el encabezado de un `if`, `for` o `switch`, Go no lee `T{` como un literal compuesto sin
// paréntesis: la llave abre el cuerpo. tree-sitter-go sí, y entonces `if x {...}` seguido de un
// bloque suelto `{...}` le queda como condición `x{...}` y cuerpo el bloque. Ese árbol no es el de
// Go: formatearlo rompería el archivo.
fn misreads_a_body_as_literal(node: Node<'_>) -> bool {
    node.kind() == COMPOSITE
        && node
            .child_by_field_name(TYPE_FIELD)
            .is_some_and(|typ| NAMED_TYPES.contains(&typ.kind()))
        && in_a_header(node)
}

// En Go, `*x.f`, `*f()` y `*x[i]` aplican el `*` a todo lo de la derecha, y `*T(x).f` desreferencia
// `T(x).f`. tree-sitter-go puede leer `*(*T)(p).f` como el campo de una conversión al tipo `*(*T)`,
// y al imprimirlo hay que encerrar la conversión entre paréntesis: el programa cambia. Sin nada a la
// derecha, `*(*T)(p)` se imprime igual y no importa.
fn misreads_a_unary_operand(node: Node<'_>) -> bool {
    POSTFIX_OPERANDS.iter().any(|(kind, field)| {
        node.kind() == *kind
            && node
                .child_by_field_name(field)
                .is_some_and(|operand| operand.kind() == UNARY || is_pointer_conversion(operand))
    })
}

fn is_pointer_conversion(node: Node<'_>) -> bool {
    node.kind() == CONVERSION
        && node
            .child_by_field_name(TYPE_FIELD)
            .is_some_and(|typ| typ.kind() == POINTER || starts_with_arrow(typ))
}

// `chan<- chan T` es un canal de envío de `chan T`: el `<-` va con el `chan` de más a la izquierda.
// tree-sitter-go lo lee como un `chan` sin dirección de `<-chan T`, que en Go pide paréntesis.
// `<-chan <-chan T` y `chan<- <-chan T` sí son eso.
fn misreads_a_send_channel(node: Node<'_>) -> bool {
    let token = |position: u32| node.child(position).map(|child| child.kind());
    node.kind() == CHANNEL
        && token(0) == Some(CHAN)
        && token(1) != Some(ARROW)
        && named_children(node).into_iter().any(starts_with_arrow)
}

fn starts_with_arrow(typ: Node<'_>) -> bool {
    typ.kind() == CHANNEL && typ.child(0).is_some_and(|first| first.kind() == ARROW)
}

// Los árboles que tree-sitter-go arma distinto que go/parser: formatearlos cambiaría el programa.
pub fn misreads_go(root: Node<'_>) -> bool {
    descendants(root).into_iter().any(|node| {
        misreads_a_body_as_literal(node) || misreads_a_unary_operand(node) || misreads_a_send_channel(node)
    })
}

fn in_a_header(literal: Node<'_>) -> bool {
    let mut child = literal;
    while let Some(parent) = child.parent() {
        if ENCLOSING.contains(&parent.kind()) {
            return false;
        }
        if HEADER_CLAUSES.contains(&parent.kind()) {
            return true;
        }
        if parent.kind() == FOR {
            return parent.child_by_field_name(BODY_FIELD) != Some(child);
        }
        if let Some((_, fields)) = HEADER_FIELDS.iter().find(|(kind, _)| *kind == parent.kind()) {
            return fields
                .iter()
                .any(|field| parent.child_by_field_name(field) == Some(child));
        }
        child = parent;
    }
    false
}
