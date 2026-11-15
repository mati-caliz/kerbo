use tree_sitter::Node;

use super::java::Position;
use crate::lint::java::text;
use crate::lint::java_calls::TYPE_DECLARATIONS;
use crate::syntax::{descendants, named_children, one_based};

const ANNOTATION_TYPE: &str = "annotation_type_declaration";
const ANNOTATION_ELEMENT: &str = "annotation_type_element_declaration";
const ANNOTATION: &str = "annotation";
const ARGUMENTS_FIELD: &str = "arguments";
const NAME_FIELD: &str = "name";
const KEY_FIELD: &str = "key";
const VALUE_FIELD: &str = "value";
const PAIR: &str = "element_value_pair";
const ARRAY: &str = "element_value_array_initializer";
const CLASS_LITERAL: &str = "class_literal";
const CONSTANT_LITERALS: [&str; 10] = [
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "string_literal",
    "character_literal",
    "true",
    "false",
];
const UNARY: &str = "unary_expression";
const OPERAND_FIELD: &str = "operand";
const OPERATOR_FIELD: &str = "operator";
const SINGLE_ELEMENT: &str = "value";
const QUALIFIER_SEPARATOR: char = '.';
const CLASS_SUFFIX: &str = ".class";

// Un elemento de una anotación del proyecto con valor por defecto: `int value() default 0;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationDefault {
    pub annotation: String,
    pub element: String,
    pub value: String,
}

// Un valor escrito al usar una anotación: `@Parameter(0)` o `@Test(expected = None.class)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationValue {
    pub written: String,
    pub enclosing: Vec<String>,
    pub element: String,
    pub value: String,
    pub position: Position,
}

pub fn annotation_defaults(root: Node<'_>, source: &[u8], package: &str) -> Vec<AnnotationDefault> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == ANNOTATION_TYPE)
        .flat_map(|declaration| {
            let mut path = enclosing_types(declaration, source);
            path.extend(name_of(declaration, source));
            let annotation = qualified(package, &path);
            declaration
                .child_by_field_name("body")
                .map(named_children)
                .unwrap_or_default()
                .into_iter()
                .filter(|member| member.kind() == ANNOTATION_ELEMENT)
                .filter_map(move |member| {
                    Some(AnnotationDefault {
                        annotation: annotation.clone(),
                        element: name_of(member, source)?,
                        value: canonical(member.child_by_field_name(VALUE_FIELD)?, source)?,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn annotation_values(root: Node<'_>, source: &[u8]) -> Vec<AnnotationValue> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == ANNOTATION)
        .flat_map(|annotation| written_values(annotation, source))
        .collect()
}

fn written_values(annotation: Node<'_>, source: &[u8]) -> Vec<AnnotationValue> {
    let (Some(name), Some(arguments)) = (
        annotation.child_by_field_name(NAME_FIELD),
        annotation.child_by_field_name(ARGUMENTS_FIELD),
    ) else {
        return Vec::new();
    };
    let written = text(name, source).to_owned();
    let enclosing = enclosing_types(annotation, source);
    named_children(arguments)
        .into_iter()
        .filter_map(|argument| {
            let (element, value) = if argument.kind() == PAIR {
                (
                    text(argument.child_by_field_name(KEY_FIELD)?, source).to_owned(),
                    argument.child_by_field_name(VALUE_FIELD)?,
                )
            } else {
                (SINGLE_ELEMENT.to_owned(), argument)
            };
            let start = value.start_position();
            Some(AnnotationValue {
                written: written.clone(),
                enclosing: enclosing.clone(),
                element,
                value: canonical(value, source)?,
                position: Position {
                    line: one_based(start.row),
                    column: one_based(start.column),
                },
            })
        })
        .collect()
}

// Como compara IntelliJ con `AnnotationUtil.equal`: dos valores son iguales si los dos son constantes
// y valen lo mismo, y los arreglos, elemento por elemento. Sin resolver nombres, una referencia
// (que puede ser una constante de enum, que no cuenta) no vale; un literal de clase vale por su
// último nombre (`Test.None.class` y `None.class` son lo mismo).
fn canonical(value: Node<'_>, source: &[u8]) -> Option<String> {
    let written = text(value, source);
    match value.kind() {
        kind if CONSTANT_LITERALS.contains(&kind) => Some(written.to_owned()),
        UNARY => {
            let operand = value.child_by_field_name(OPERAND_FIELD)?;
            let operator = text(value.child_by_field_name(OPERATOR_FIELD)?, source);
            Some(format!("{operator}{}", canonical(operand, source)?))
        }
        CLASS_LITERAL => {
            let class_name = written.trim_end_matches(CLASS_SUFFIX).trim();
            Some(format!("{}{CLASS_SUFFIX}", last_name(class_name)))
        }
        ARRAY => {
            let elements = named_children(value)
                .into_iter()
                .map(|element| canonical(element, source))
                .collect::<Option<Vec<String>>>()?;
            Some(format!("{{{}}}", elements.join(",")))
        }
        _ => None,
    }
}

fn last_name(written: &str) -> &str {
    written
        .rsplit(QUALIFIER_SEPARATOR)
        .next()
        .unwrap_or(written)
        .trim()
}

fn enclosing_types(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut names: Vec<String> = std::iter::successors(node.parent(), Node::parent)
        .filter(|ancestor| TYPE_DECLARATIONS.contains(&ancestor.kind()))
        .filter_map(|ancestor| name_of(ancestor, source))
        .collect();
    names.reverse();
    names
}

fn name_of(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source).to_owned())
}

pub(super) fn qualified(package: &str, path: &[String]) -> String {
    std::iter::once(package)
        .filter(|prefix| !prefix.is_empty())
        .chain(path.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(".")
}
