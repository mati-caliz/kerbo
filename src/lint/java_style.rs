use tree_sitter::Node;

use super::java::{
    CONSTRUCTOR, FINAL, METHOD, NAME_FIELD, STATIC, TYPE_DECLARATIONS, VARIABLE_DECLARATOR, modifiers, text,
};
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::limits::Preset;
use crate::rules::{JavaRule, RuleId};
use crate::syntax::named_children;

const FIELD: &str = "field_declaration";
const INTERFACE_FIELD: &str = "constant_declaration";
const FORMAL_PARAMETERS: &str = "formal_parameters";
const FORMAL_PARAMETER: &str = "formal_parameter";
const SPREAD_PARAMETER: &str = "spread_parameter";
const MARKER_ANNOTATION: &str = "marker_annotation";
const ANNOTATION: &str = "annotation";
const OVERRIDE_NAMES: &[&str] = &["Override", "java.lang.Override"];
const SERIALIZATION_FIELDS: &[&str] = &["serialVersionUID", "serialPersistentFields"];
const INTEGER_LITERALS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
];
const LOWER_ELL: char = 'l';
const UPPER_ELL: &str = "L";
const SEMICOLON: &str = ";";
const STATEMENT_LISTS: &[&str] = &["block", "constructor_body", "switch_block_statement_group"];
const BODY_FIELDS: &[&str] = &["body", "consequence", "alternative"];
const WORD_SEPARATOR: char = '_';

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for &node in nodes {
        let kind = node.kind();
        if TYPE_DECLARATIONS.contains(&kind) {
            findings.extend(named(node, source, JavaRule::TypeName, is_pascal_case));
            findings.extend(methods_named_like(node, source));
        } else if kind == METHOD {
            findings.extend(method_name(node, source));
            findings.extend(parameter_names(node, source));
        } else if kind == CONSTRUCTOR {
            findings.extend(parameter_names(node, source));
        } else if kind == FIELD || kind == INTERFACE_FIELD {
            findings.extend(field_names(node, source));
        } else if INTEGER_LITERALS.contains(&kind) {
            findings.extend(lowercase_ell(node, source));
        }
        findings.extend(
            empty_statements(node)
                .into_iter()
                .map(|semicolon| finding(LintKind::Java(JavaRule::EmptyStatement), semicolon, "", None)),
        );
    }
    findings
}

fn named(node: Node<'_>, source: &[u8], rule: JavaRule, accepts: fn(&str) -> bool) -> Option<LintFinding> {
    let name = node.child_by_field_name(NAME_FIELD)?;
    let written = text(name, source);
    (!accepts(written)).then(|| finding(LintKind::Java(rule), name, written, None))
}

// MethodName no mira lo que lleva @Override, y marca además el método que se llama como su clase
// (eso se mira desde la clase: subir al padre en tree-sitter es caro).
fn method_name(method: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    if overrides(method, source) {
        return None;
    }
    let name = method.child_by_field_name(NAME_FIELD)?;
    let written = text(name, source);
    (!is_camel_case(written)).then(|| finding(LintKind::Java(JavaRule::MethodName), name, written, None))
}

fn methods_named_like(declaration: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let Some(type_name) = declaration
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source))
    else {
        return Vec::new();
    };
    declaration
        .child_by_field_name("body")
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|member| member.kind() == METHOD && !overrides(*member, source))
        .filter_map(|method| method.child_by_field_name(NAME_FIELD))
        .filter(|name| text(*name, source) == type_name && is_camel_case(type_name))
        .map(|name| finding(LintKind::Java(JavaRule::MethodName), name, type_name, None))
        .collect()
}

fn overrides(method: Node<'_>, source: &[u8]) -> bool {
    modifiers(method).is_some_and(|modifiers| {
        named_children(modifiers).into_iter().any(|annotation| {
            (annotation.kind() == MARKER_ANNOTATION || annotation.kind() == ANNOTATION)
                && annotation
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| OVERRIDE_NAMES.contains(&text(name, source)))
        })
    })
}

fn parameter_names(callable: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let Some(parameters) = named_children(callable)
        .into_iter()
        .find(|child| child.kind() == FORMAL_PARAMETERS)
    else {
        return Vec::new();
    };
    named_children(parameters)
        .into_iter()
        .filter_map(|parameter| match parameter.kind() {
            FORMAL_PARAMETER => Some(parameter),
            SPREAD_PARAMETER => named_children(parameter)
                .into_iter()
                .find(|child| child.kind() == VARIABLE_DECLARATOR),
            _ => None,
        })
        .filter_map(|declared| named(declared, source, JavaRule::ParameterName, is_camel_case))
        .collect()
}

// Un campo de interfaz es static final aunque no lo diga.
fn field_names(field: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let written: Vec<&str> = modifiers(field)
        .map(|modifiers| {
            super::java::children(modifiers)
                .into_iter()
                .map(|modifier| text(modifier, source))
                .collect()
        })
        .unwrap_or_default();
    let constant = field.kind() == INTERFACE_FIELD || (written.contains(&STATIC) && written.contains(&FINAL));
    let (rule, accepts): (JavaRule, fn(&str) -> bool) = if constant {
        (JavaRule::ConstantName, is_constant_case)
    } else if written.contains(&STATIC) {
        (JavaRule::StaticVariableName, is_camel_case)
    } else {
        (JavaRule::MemberName, is_camel_case)
    };
    named_children(field)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter(|declarator| {
            !(constant
                && declarator
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| SERIALIZATION_FIELDS.contains(&text(name, source))))
        })
        .filter_map(|declarator| named(declarator, source, rule, accepts))
        .collect()
}

fn lowercase_ell(literal: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let written = text(literal, source);
    let digits = written.strip_suffix(LOWER_ELL)?;
    let fix = Fix::replace(literal, &format!("{digits}{UPPER_ELL}"), source);
    Some(finding(
        LintKind::Java(JavaRule::UpperEll),
        literal,
        written,
        Some(fix),
    ))
}

// Un `;` suelto en una lista de sentencias o como cuerpo de un if, un for o un while. Se busca
// desde el contenedor: subir al padre en tree-sitter es caro.
fn empty_statements(container: Node<'_>) -> Vec<Node<'_>> {
    let is_semicolon = |node: &Node<'_>| node.kind() == SEMICOLON && !node.is_named();
    if STATEMENT_LISTS.contains(&container.kind()) {
        return super::java::children(container)
            .into_iter()
            .filter(is_semicolon)
            .collect();
    }
    BODY_FIELDS
        .iter()
        .filter_map(|field| container.child_by_field_name(field))
        .filter(is_semicolon)
        .collect()
}

fn is_pascal_case(name: &str) -> bool {
    starts_with_and_continues(name, |first| first.is_ascii_uppercase())
}

fn is_camel_case(name: &str) -> bool {
    starts_with_and_continues(name, |first| first.is_ascii_lowercase())
}

fn starts_with_and_continues(name: &str, first_allowed: fn(char) -> bool) -> bool {
    let mut characters = name.chars();
    characters.next().is_some_and(first_allowed)
        && characters.all(|character| character.is_ascii_alphanumeric())
}

fn is_constant_case(name: &str) -> bool {
    let mut words = name.split(WORD_SEPARATOR);
    let first_starts_with_letter = name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase());
    first_starts_with_letter
        && words.all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        })
}

pub fn relaxed_by(preset: Preset) -> Vec<RuleId> {
    match preset {
        Preset::Strict | Preset::Recommended => Vec::new(),
        Preset::Minimal => JavaRule::ALL.into_iter().map(RuleId::Java).collect(),
    }
}
