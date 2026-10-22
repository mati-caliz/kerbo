use tree_sitter::Node;

use super::java::{NAME_FIELD, modifiers, text};
use super::java_receivers::simple_type_name;
use crate::syntax::named_children;

const ANNOTATIONS: &[&str] = &["marker_annotation", "annotation"];
const ENUM_BODY: &str = "enum_body";
const CLASS: &str = "class_declaration";
const INTERFACE: &str = "interface_declaration";
const ENUM: &str = "enum_declaration";
const RECORD: &str = "record_declaration";
const CREATION: &str = "object_creation_expression";
const TYPE_FIELD: &str = "type";
const ANNOTATION_TYPE: &str = "annotation_type_declaration";
const MEMBER_CONTAINERS: &[&str] = &[
    "program",
    "class_body",
    "interface_body",
    "enum_body_declarations",
    "annotation_type_body",
];
const STRING_LITERAL: &str = "string_literal";
const STRING_FRAGMENT: &str = "string_fragment";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const QUALIFIER_SEPARATOR: char = '.';
const IDENTIFIER_SEPARATOR: char = '_';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerKind {
    Class,
    Interface,
    Enum,
    Record,
    Annotation,
    Local,
}

// El tipo que declara un método: qué es y con qué anotaciones. Una clase anónima, la de una
// constante de un enum o una declarada dentro de un método cuentan como locales.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pub kind: OwnerKind,
    pub name: String,
    pub annotations: Vec<String>,
}

pub fn owner_of(member: Node<'_>, source: &[u8]) -> Owner {
    let mut container = member.parent().and_then(|body| body.parent());
    if container.is_some_and(|node| node.kind() == ENUM_BODY) {
        container = container.and_then(|enum_body| enum_body.parent());
    }
    let local = Owner {
        kind: OwnerKind::Local,
        name: String::new(),
        annotations: Vec::new(),
    };
    let Some(declaration) = container else {
        return local;
    };
    if declaration.kind() == CREATION {
        return Owner {
            name: declaration
                .child_by_field_name(TYPE_FIELD)
                .map(|written| simple_type_name(text(written, source)))
                .unwrap_or_default(),
            ..local
        };
    }
    let kind = match declaration.kind() {
        CLASS => OwnerKind::Class,
        INTERFACE => OwnerKind::Interface,
        ENUM => OwnerKind::Enum,
        RECORD => OwnerKind::Record,
        ANNOTATION_TYPE => OwnerKind::Annotation,
        _ => return local,
    };
    let is_member = declaration
        .parent()
        .is_some_and(|parent| MEMBER_CONTAINERS.contains(&parent.kind()));
    if !is_member {
        return local;
    }
    Owner {
        kind,
        name: declaration
            .child_by_field_name(NAME_FIELD)
            .map(|name| text(name, source).to_owned())
            .unwrap_or_default(),
        annotations: annotation_names(declaration, source),
    }
}

// Los nombres simples de las anotaciones de una declaración (`@org.junit.Test` es `Test`).
pub fn annotation_names(declaration: Node<'_>, source: &[u8]) -> Vec<String> {
    modifiers(declaration)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|child| ANNOTATIONS.contains(&child.kind()))
        .filter_map(|annotation| annotation.child_by_field_name(NAME_FIELD))
        .map(|name| {
            let written = text(name, source);
            written
                .rsplit(QUALIFIER_SEPARATOR)
                .next()
                .unwrap_or(written)
                .to_owned()
        })
        .collect()
}

// Un texto que es un nombre de Java (`"argumentos"` en `@MethodSource("argumentos")`, o el nombre
// que se busca por reflexión) cuenta como uso de lo que se llame así. El que inicializa a una
// variable de su mismo nombre (`String PAGADO = "PAGADO"`) no nombra a nadie más.
pub fn mentioned_name(literal: Node<'_>, source: &[u8]) -> Option<String> {
    if literal.kind() != STRING_LITERAL {
        return None;
    }
    let fragments = named_children(literal);
    let [fragment] = fragments.as_slice() else {
        return None;
    };
    let content = (fragment.kind() == STRING_FRAGMENT).then(|| text(*fragment, source))?;
    let mut characters = content.chars();
    let starts_like_a_name = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == IDENTIFIER_SEPARATOR);
    (starts_like_a_name
        && characters.all(|letter| letter.is_ascii_alphanumeric() || letter == IDENTIFIER_SEPARATOR)
        && !initializes_itself(literal, content, source))
    .then(|| content.to_owned())
}

fn initializes_itself(literal: Node<'_>, content: &str, source: &[u8]) -> bool {
    literal
        .parent()
        .filter(|parent| parent.kind() == VARIABLE_DECLARATOR)
        .and_then(|declarator| declarator.child_by_field_name(NAME_FIELD))
        .is_some_and(|name| text(name, source) == content)
}
