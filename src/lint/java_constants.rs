use std::collections::HashMap;

use tree_sitter::Node;

use super::java::{NAME_FIELD, STATIC, text};
use super::java_calls::{TYPE_DECLARATIONS, Visibility, has_modifier, visibility};
use crate::syntax::named_children;

const INTERFACE: &str = "interface_declaration";
const FIELD: &str = "field_declaration";
const INTERFACE_FIELD: &str = "constant_declaration";
const ENUM_CONSTANT: &str = "enum_constant";
const ENUM_BODY_DECLARATIONS: &str = "enum_body_declarations";
const FINAL: &str = "final";
const BODY_FIELD: &str = "body";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const IDENTIFIER: &str = "identifier";
const FIELD_ACCESS: &str = "field_access";
const UNARY: &str = "unary_expression";
const MINUS: &str = "-";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const LITERALS: &[&str] = &[
    "string_literal",
    "character_literal",
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "true",
    "false",
    "null_literal",
];
const NUMERIC_LITERALS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
];
const QUALIFIER_SEPARATOR: char = '.';

// Por nombre, las clases de este archivo que declaran una constante con ese nombre: un campo
// static final, uno de una interfaz o una constante de un enum.
pub type Constants = HashMap<String, Vec<String>>;

// Una constante del archivo: la clase que la declara, el nombre y quién la ve. Las de un enum y las
// de una interfaz son públicas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constant {
    pub owner: String,
    pub name: String,
    pub visibility: Visibility,
}

pub fn constants_in(nodes: &[Node<'_>], source: &[u8]) -> Constants {
    let mut constants: Constants = HashMap::new();
    for constant in declared_constants(nodes, source) {
        constants.entry(constant.name).or_default().push(constant.owner);
    }
    constants
}

pub fn declared_constants(nodes: &[Node<'_>], source: &[u8]) -> Vec<Constant> {
    let mut constants = Vec::new();
    for &declaration in nodes
        .iter()
        .filter(|node| TYPE_DECLARATIONS.contains(&node.kind()))
    {
        let Some(owner) = declaration
            .child_by_field_name(NAME_FIELD)
            .map(|name| text(name, source))
        else {
            continue;
        };
        let Some(body) = declaration.child_by_field_name(BODY_FIELD) else {
            continue;
        };
        let interface = declaration.kind() == INTERFACE;
        let members = named_children(body).into_iter().flat_map(|member| {
            if member.kind() == ENUM_BODY_DECLARATIONS {
                named_children(member)
            } else {
                vec![member]
            }
        });
        for member in members {
            let visibility =
                if interface || member.kind() == ENUM_CONSTANT || member.kind() == INTERFACE_FIELD {
                    Visibility::Public
                } else {
                    visibility(member, source)
                };
            let names: Vec<&str> = match member.kind() {
                ENUM_CONSTANT => member
                    .child_by_field_name(NAME_FIELD)
                    .map(|name| text(name, source))
                    .into_iter()
                    .collect(),
                FIELD | INTERFACE_FIELD
                    if interface
                        || member.kind() == INTERFACE_FIELD
                        || (has_modifier(member, STATIC, source) && has_modifier(member, FINAL, source)) =>
                {
                    named_children(member)
                        .into_iter()
                        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                        .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
                        .map(|name| text(name, source))
                        .collect()
                }
                _ => Vec::new(),
            };
            constants.extend(names.into_iter().map(|name| Constant {
                owner: owner.to_owned(),
                name: name.to_owned(),
                visibility,
            }));
        }
    }
    constants
}

// El valor como lo escribe IntelliJ: el literal tal cual, y una constante con su clase delante
// (`Clase.NOMBRE`). Lo demás no es constante.
pub fn constant_value(argument: Node<'_>, source: &[u8], constants: &Constants) -> Option<String> {
    let kind = argument.kind();
    if LITERALS.contains(&kind) {
        return Some(text(argument, source).to_owned());
    }
    match kind {
        UNARY => {
            let operand = argument.child_by_field_name("operand")?;
            let operator = argument.child_by_field_name("operator")?;
            (operator.kind() == MINUS && NUMERIC_LITERALS.contains(&operand.kind()))
                .then(|| format!("{MINUS}{}", text(operand, source)))
        }
        IDENTIFIER => {
            let name = text(argument, source);
            match constants.get(name).map(Vec::as_slice) {
                Some([owner]) => Some(format!("{owner}{QUALIFIER_SEPARATOR}{name}")),
                _ => None,
            }
        }
        FIELD_ACCESS => {
            let owner = argument.child_by_field_name(OBJECT_FIELD)?;
            let name = text(argument.child_by_field_name(FIELD_FIELD)?, source);
            let owner_name = text(owner, source);
            let simple_owner = owner_name
                .rsplit(QUALIFIER_SEPARATOR)
                .next()
                .unwrap_or(owner_name);
            let known = constants
                .get(name)
                .is_some_and(|owners| owners.iter().any(|known| known == simple_owner));
            let looks_constant = simple_owner.starts_with(|first: char| first.is_ascii_uppercase())
                && name
                    .chars()
                    .all(|letter| letter.is_ascii_uppercase() || letter.is_ascii_digit() || letter == '_');
            (known || looks_constant).then(|| format!("{simple_owner}{QUALIFIER_SEPARATOR}{name}"))
        }
        _ => None,
    }
}

// SameReturnValue sólo mira literales: un método que devuelve una constante con nombre es su
// forma de exponerla.
pub fn literal_value(value: Node<'_>, source: &[u8]) -> Option<String> {
    let kind = value.kind();
    if LITERALS.contains(&kind) {
        return Some(text(value, source).to_owned());
    }
    let operand = value.child_by_field_name("operand")?;
    let operator = value.child_by_field_name("operator")?;
    (kind == UNARY && operator.kind() == MINUS && NUMERIC_LITERALS.contains(&operand.kind()))
        .then(|| format!("{MINUS}{}", text(operand, source)))
}
