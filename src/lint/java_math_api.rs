use tree_sitter::Node;

use super::java::{
    IDENTIFIER, NAME_FIELD, OBJECT_CREATION, TYPE_FIELD, is_type_declaration, text, unparenthesized,
};
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const MATH_CLAMP_RELEASE: u32 = 21;
const BUILDER_REPEAT_RELEASE: u32 = 21;
const INVOCATION: &str = "method_invocation";
const FIELD_ACCESS: &str = "field_access";
const STRING_LITERAL: &str = "string_literal";
const UNARY: &str = "unary_expression";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const ARGUMENTS_FIELD: &str = "arguments";
const OPERAND_FIELD: &str = "operand";
const COMMENT_SUFFIX: &str = "comment";
const MATH_RECEIVERS: [&str; 2] = ["Math", "java.lang.Math"];
const MATH_CLASS: &str = "Math";
const MIN: &str = "min";
const MAX: &str = "max";
const APPEND: &str = "append";
const REPEAT: &str = "repeat";
const BUILDERS: [&str; 2] = ["StringBuilder", "StringBuffer"];
const STRINGS: [&str; 2] = ["String", "java.lang.String"];
const NUMERIC_LITERALS: [&str; 7] = [
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "character_literal",
];
const TYPE_ARGUMENTS_START: char = '<';
const CONSTANT_NAME_SEPARATOR: char = '_';

pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    let since = |needed: u32| release.is_none_or(|release| release >= needed);
    let declares_math = descendants(root).into_iter().any(|node| {
        is_type_declaration(node)
            && node
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == MATH_CLASS)
    });
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == INVOCATION)
        .filter_map(|call| {
            if since(MATH_CLAMP_RELEASE) && !declares_math && can_clamp(call, source) {
                return Some(finding(
                    LintKind::Java(JavaRule::MathClampMigration),
                    call,
                    text(call, source),
                    None,
                ));
            }
            let name = call.child_by_field_name(NAME_FIELD)?;
            (since(BUILDER_REPEAT_RELEASE) && appends_a_repeat(call, source)).then(|| {
                finding(
                    LintKind::Java(JavaRule::StringRepeatCanBeUsed),
                    name,
                    text(call, source),
                    None,
                )
            })
        })
        .collect()
}

fn arguments(call: Node<'_>) -> Vec<Node<'_>> {
    call.child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !argument.kind().ends_with(COMMENT_SUFFIX))
        .map(unparenthesized)
        .collect()
}

fn math_function<'call>(call: Node<'_>, source: &'call [u8]) -> Option<&'call str> {
    if call.kind() != INVOCATION || arguments(call).len() != 2 {
        return None;
    }
    let receiver = call.child_by_field_name(OBJECT_FIELD)?;
    let name = text(call.child_by_field_name(NAME_FIELD)?, source);
    (MATH_RECEIVERS.contains(&text(receiver, source)) && (name == MIN || name == MAX)).then_some(name)
}

// `MathClampMigration` de IntelliJ en el modo por defecto: `Math.min` o `Math.max` con una llamada a la
// otra entre sus argumentos, salvo que todo lo que quedaría dentro de esa llamada sea constante.
fn can_clamp(call: Node<'_>, source: &[u8]) -> bool {
    let Some(function) = math_function(call, source) else {
        return false;
    };
    let other = if function == MIN { MAX } else { MIN };
    let nested: Vec<Node<'_>> = arguments(call)
        .into_iter()
        .filter(|argument| math_function(*argument, source) == Some(other))
        .collect();
    !nested.is_empty()
        && nested
            .iter()
            .flat_map(|inner| leaves(*inner, other, source))
            .any(|leaf| !is_constant(leaf, source))
}

fn leaves<'tree>(call: Node<'tree>, function: &str, source: &[u8]) -> Vec<Node<'tree>> {
    arguments(call)
        .into_iter()
        .flat_map(|argument| {
            if math_function(argument, source) == Some(function) {
                leaves(argument, function, source)
            } else {
                vec![argument]
            }
        })
        .collect()
}

fn is_constant(value: Node<'_>, source: &[u8]) -> bool {
    match value.kind() {
        kind if NUMERIC_LITERALS.contains(&kind) => true,
        UNARY => value
            .child_by_field_name(OPERAND_FIELD)
            .is_some_and(|operand| is_constant(unparenthesized(operand), source)),
        IDENTIFIER => is_constant_name(text(value, source)),
        FIELD_ACCESS => value
            .child_by_field_name(FIELD_FIELD)
            .is_some_and(|field| is_constant_name(text(field, source))),
        _ => false,
    }
}

fn is_constant_name(name: &str) -> bool {
    name.chars().any(|letter| letter.is_ascii_uppercase())
        && name.chars().all(|letter| {
            letter.is_ascii_uppercase() || letter.is_ascii_digit() || letter == CONSTANT_NAME_SEPARATOR
        })
}

// `StringRepeatCanBeUsed` de IntelliJ, la forma de Java 21: `builder.append(texto.repeat(n))`, que
// `StringBuilder.repeat(texto, n)` hace sin armar el texto intermedio.
fn appends_a_repeat(call: Node<'_>, source: &[u8]) -> bool {
    let is_append = call
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == APPEND);
    let [argument] = arguments(call)[..] else {
        return false;
    };
    let repeats = argument.kind() == INVOCATION
        && argument
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == REPEAT)
        && arguments(argument).len() == 1
        && argument
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|text_receiver| is_string(unparenthesized(text_receiver), source));
    is_append
        && repeats
        && call
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|builder| is_builder(unparenthesized(builder), source))
}

pub(super) fn is_builder(receiver: Node<'_>, source: &[u8]) -> bool {
    match receiver.kind() {
        IDENTIFIER => {
            declared_simple(receiver, source).is_some_and(|written| BUILDERS.contains(&written.as_str()))
        }
        OBJECT_CREATION => receiver
            .child_by_field_name(TYPE_FIELD)
            .is_some_and(|written| BUILDERS.contains(&text(written, source))),
        INVOCATION => {
            receiver
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == APPEND)
                && receiver
                    .child_by_field_name(OBJECT_FIELD)
                    .is_some_and(|inner| is_builder(unparenthesized(inner), source))
        }
        _ => false,
    }
}

fn is_string(receiver: Node<'_>, source: &[u8]) -> bool {
    match receiver.kind() {
        STRING_LITERAL => true,
        IDENTIFIER => {
            declared_simple(receiver, source).is_some_and(|written| STRINGS.contains(&written.as_str()))
        }
        _ => false,
    }
}

fn declared_simple(variable: Node<'_>, source: &[u8]) -> Option<String> {
    let written = declared_type(variable, text(variable, source), source)?;
    Some(
        text(written, source)
            .split(TYPE_ARGUMENTS_START)
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned(),
    )
}
