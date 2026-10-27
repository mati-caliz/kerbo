use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::constants::constant_text;
use super::spec_names;
use crate::syntax::{descendants, named_children};

const IDENTIFIER: &str = "identifier";
const BINARY: &str = "binary_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const CONST_DECLARATION: &str = "const_declaration";
const CONST_SPEC: &str = "const_spec";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const PLUS: &str = "+";
const LOCAL_DECLARATIONS: &[&str] = &[
    "var_spec",
    "parameter_declaration",
    "variadic_parameter_declaration",
    "short_var_declaration",
    "range_clause",
    "type_switch_statement",
];
const TOP_LEVEL_FUNCTIONS: &[&str] = &["function_declaration", "method_declaration"];
const DECLARED_FIELDS: &[&str] = &["name", "left", "alias"];

// Un trozo de un mensaje que el archivo no puede resolver solo: texto o el nombre de una constante
// que se busca en el resto del paquete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagePiece {
    Text(String),
    Constant(String),
}

// Un mensaje de `errors.New` o `fmt.Errorf` que nombra constantes de otro archivo del paquete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMessage {
    pub line: usize,
    pub column: usize,
    pub callee: String,
    pub pieces: Vec<MessagePiece>,
}

const CONSTANT_SEPARATOR: &str = "+";

impl PendingMessage {
    // El sujeto del aviso mientras falta resolverlo: las constantes que nombra.
    pub fn constant_names(&self) -> String {
        self.pieces
            .iter()
            .filter_map(|piece| match piece {
                MessagePiece::Constant(name) => Some(name.as_str()),
                MessagePiece::Text(_) => None,
            })
            .collect::<Vec<_>>()
            .join(CONSTANT_SEPARATOR)
    }

    // El texto con las constantes del paquete; si alguna no está, no es un mensaje constante.
    pub fn resolve(&self, constants: &HashMap<&str, &str>) -> Option<String> {
        self.pieces
            .iter()
            .map(|piece| match piece {
                MessagePiece::Text(text) => Some(text.as_str()),
                MessagePiece::Constant(name) => constants.get(name.as_str()).copied(),
            })
            .collect()
    }
}

pub struct FileNames<'tree> {
    pub constants: HashMap<String, Option<Node<'tree>>>,
}

// Las piezas del mensaje si sólo falta resolver constantes; un nombre que el archivo declara como
// variable o parámetro en la función que lo usa no es una constante del paquete.
pub fn message_pieces(node: Node<'_>, source: &[u8], names: &FileNames<'_>) -> Option<Vec<MessagePiece>> {
    if let Some(text) = constant_text(node, source, &names.constants, 0) {
        return Some(vec![MessagePiece::Text(text)]);
    }
    match node.kind() {
        PARENTHESIZED => message_pieces(named_children(node).into_iter().next()?, source, names),
        BINARY => {
            if node.child_by_field_name(OPERATOR_FIELD)?.utf8_text(source).ok()? != PLUS {
                return None;
            }
            let mut pieces = message_pieces(node.child_by_field_name(LEFT_FIELD)?, source, names)?;
            pieces.extend(message_pieces(
                node.child_by_field_name(RIGHT_FIELD)?,
                source,
                names,
            )?);
            Some(pieces)
        }
        IDENTIFIER => {
            let name = node.utf8_text(source).ok()?;
            (!names.constants.contains_key(name) && !declared_in_function(node, source).contains(name))
                .then(|| vec![MessagePiece::Constant(name.to_owned())])
        }
        _ => None,
    }
}

pub fn has_constants(pieces: &[MessagePiece]) -> bool {
    pieces
        .iter()
        .any(|piece| matches!(piece, MessagePiece::Constant(_)))
}

// Las constantes de texto del paquete que declara el archivo, con su valor si se resuelve en él.
pub fn package_constants(root: Node<'_>, source: &[u8], names: &FileNames<'_>) -> Vec<(String, String)> {
    named_children(root)
        .into_iter()
        .filter(|declaration| declaration.kind() == CONST_DECLARATION)
        .flat_map(descendants)
        .filter(|spec| spec.kind() == CONST_SPEC)
        .flat_map(|spec| {
            let values = spec
                .child_by_field_name(VALUE_FIELD)
                .map(named_children)
                .unwrap_or_default();
            spec_names(spec)
                .into_iter()
                .zip(values)
                .filter_map(|(name, value)| {
                    let text = constant_text(value, source, &names.constants, 0)?;
                    Some((name.utf8_text(source).ok()?.to_owned(), text))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn declared_in_function(node: Node<'_>, source: &[u8]) -> HashSet<String> {
    let mut function = None;
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if TOP_LEVEL_FUNCTIONS.contains(&ancestor.kind()) {
            function = Some(ancestor);
        }
        current = ancestor.parent();
    }
    function
        .map(|function| declared_names(function, source))
        .unwrap_or_default()
}

fn declared_names(scope: Node<'_>, source: &[u8]) -> HashSet<String> {
    descendants(scope)
        .into_iter()
        .filter(|node| LOCAL_DECLARATIONS.contains(&node.kind()))
        .flat_map(|declaration| {
            let mut cursor = declaration.walk();
            declaration
                .children(&mut cursor)
                .enumerate()
                .filter(|(index, _)| {
                    u32::try_from(*index)
                        .ok()
                        .and_then(|index| declaration.field_name_for_child(index))
                        .is_some_and(|field| DECLARED_FIELDS.contains(&field))
                })
                .map(|(_, child)| child)
                .collect::<Vec<_>>()
        })
        .flat_map(|declared| {
            if declared.kind() == IDENTIFIER {
                vec![declared]
            } else {
                named_children(declared)
                    .into_iter()
                    .filter(|child| child.kind() == IDENTIFIER)
                    .collect()
            }
        })
        .filter_map(|identifier| identifier.utf8_text(source).ok().map(str::to_owned))
        .collect()
}
