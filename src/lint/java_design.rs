use tree_sitter::Node;

use super::java::{
    ABSTRACT, CLASS, CONSTRUCTOR, FINAL, METHOD, NAME_FIELD, PRIVATE, PROTECTED, PUBLIC, STATIC,
    TYPE_DECLARATIONS, VARIABLE_DECLARATOR, children, modifiers, text,
};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

const FIELD: &str = "field_declaration";
const LOCAL: &str = "local_variable_declaration";
const ENHANCED_FOR: &str = "enhanced_for_statement";
const RESOURCE: &str = "resource";
const CLASS_BODY: &str = "class_body";
const ENUM_BODY_DECLARATIONS: &str = "enum_body_declarations";
const ANNOTATIONS: &[&str] = &["marker_annotation", "annotation"];
const BLOCK_COMMENT: &str = "block_comment";
const LINE_COMMENT: &str = "line_comment";
const JAVADOC_OPENER: &str = "/**";
const BODY_FIELD: &str = "body";
const IGNORED_EXTENSION_ANNOTATIONS: &[&str] = &["Test", "Before", "After", "BeforeClass", "AfterClass"];
const VISIBLE_FIELD_ANNOTATIONS: &[&str] = &[
    "org.junit.Rule",
    "org.junit.jupiter.api.io.TempDir",
    "org.junit.ClassRule",
    "com.google.common.annotations.VisibleForTesting",
];
const IMPORT: &str = "import_declaration";
const PACKAGE: &str = "package_declaration";
const TYPE_PARAMETERS: &str = "type_parameters";
const TYPE_FIELD: &str = "type";
const GENERIC_TYPE: &str = "generic_type";
const SCOPED_TYPE: &str = "scoped_type_identifier";
const DOT: &str = ".";
const PUBLIC_MEMBER_NAME: &str = "serialVersionUID";
const PATH_SEPARATOR: char = '.';
const WORD_SEPARATOR: char = '_';
const NATIVE: &str = "native";
const INTERFACE_BODY: &str = "interface_body";
const ANNOTATION_BODY: &str = "annotation_type_body";
const LOCAL_SCOPES: &[&str] = &["object_creation_expression", "block", "constructor_body"];

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    let imports = imported(nodes, source);
    let mut findings = Vec::new();
    for &node in nodes {
        match node.kind() {
            CLASS => findings.extend(extension_points(node, source)),
            CLASS_BODY | ENUM_BODY_DECLARATIONS => findings.extend(
                named_children(node)
                    .into_iter()
                    .filter(|member| member.kind() == FIELD)
                    .flat_map(|field| visible_fields(field, source, &imports)),
            ),
            LOCAL | RESOURCE | ENHANCED_FOR => findings.extend(local_final_names(node, source)),
            _ => {}
        }
    }
    findings
}

pub(super) fn has_modifier(node: Node<'_>, modifier: &str, source: &[u8]) -> bool {
    modifiers(node).is_some_and(|modifiers| {
        children(modifiers)
            .into_iter()
            .any(|child| text(child, source) == modifier)
    })
}

fn annotation_names<'source>(node: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    modifiers(node)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|child| ANNOTATIONS.contains(&child.kind()))
        .filter_map(|annotation| annotation.child_by_field_name(NAME_FIELD))
        .map(|name| text(name, source))
        .collect()
}

// Los nombres importados y, como Checkstyle, el paquete propio: sus anotaciones no se importan.
fn imported(nodes: &[Node<'_>], source: &[u8]) -> Vec<String> {
    let mut imports = Vec::new();
    for node in nodes.iter().copied() {
        let Some(path) = named_children(node).into_iter().next() else {
            continue;
        };
        match node.kind() {
            IMPORT => imports.push(text(path, source).to_owned()),
            PACKAGE => imports.extend(
                VISIBLE_FIELD_ANNOTATIONS
                    .iter()
                    .filter(|canonical| {
                        canonical
                            .rsplit_once(PATH_SEPARATOR)
                            .is_some_and(|(package, _)| package == text(path, source))
                    })
                    .map(|canonical| (*canonical).to_owned()),
            ),
            _ => {}
        }
    }
    imports
}

// Calca DesignForExtension: un método que no es privado, static, final ni abstracto, con cuerpo,
// en una clase que se puede heredar (no final, con algún constructor que no es privado) y que se ve
// desde afuera del paquete (ella y todas las que la encierran son públicas o protegidas), pide
// Javadoc o una de las anotaciones de test. La visibilidad que cuenta es la de las clases: un
// método de paquete también se marca. Checkstyle encuentra el Javadoc aunque haya comentarios
// comunes en el medio; se sigue al recorrer los miembros en orden.
fn extension_points(class: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    if !can_be_subclassed(class, source) {
        return findings;
    }
    let mut documented = false;
    let mut visible: Option<bool> = None;
    for member in class
        .child_by_field_name(BODY_FIELD)
        .map(named_children)
        .unwrap_or_default()
    {
        match member.kind() {
            BLOCK_COMMENT if text(member, source).starts_with(JAVADOC_OPENER) => documented = true,
            BLOCK_COMMENT | LINE_COMMENT => {}
            METHOD => {
                if !(documented && javadoc_is_seen(member))
                    && is_extension_point(member, source)
                    && *visible.get_or_insert_with(|| visible_outside_package(class, source))
                {
                    findings.push(finding(
                        LintKind::Java(JavaRule::DesignForExtension),
                        reported_position(member),
                        "",
                        None,
                    ));
                }
                documented = false;
            }
            _ => documented = false,
        }
    }
    findings
}

// Checkstyle busca el Javadoc en los modificadores o en el tipo de retorno: si el método no tiene
// modificadores y arranca con parámetros de tipo, no lo ve.
fn javadoc_is_seen(method: Node<'_>) -> bool {
    modifiers(method).is_some()
        || !named_children(method)
            .iter()
            .any(|child| child.kind() == TYPE_PARAMETERS)
}

// Sin modificadores ni parámetros de tipo, Checkstyle ubica el método donde ubica su tipo de
// retorno: en un nombre calificado (`java.util.List`), el último punto.
fn reported_position(method: Node<'_>) -> Node<'_> {
    if modifiers(method).is_some()
        || named_children(method)
            .iter()
            .any(|child| child.kind() == TYPE_PARAMETERS)
    {
        return method;
    }
    let mut written_type = method.child_by_field_name(TYPE_FIELD);
    while let Some(node) = written_type.filter(|node| node.kind() == GENERIC_TYPE) {
        written_type = named_children(node).into_iter().next();
    }
    written_type
        .filter(|node| node.kind() == SCOPED_TYPE)
        .and_then(|scoped| children(scoped).into_iter().find(|child| child.kind() == DOT))
        .unwrap_or(method)
}

fn is_extension_point(method: Node<'_>, source: &[u8]) -> bool {
    let fixed = [PRIVATE, STATIC, FINAL, ABSTRACT]
        .iter()
        .any(|modifier| has_modifier(method, modifier, source));
    let empty = method.child_by_field_name(BODY_FIELD).is_none_or(|body| {
        named_children(body)
            .iter()
            .all(|statement| statement.kind() == LINE_COMMENT || statement.kind() == BLOCK_COMMENT)
    }) && !has_modifier(method, NATIVE, source);
    let annotated = annotation_names(method, source).into_iter().any(|name| {
        IGNORED_EXTENSION_ANNOTATIONS.contains(&name.rsplit(PATH_SEPARATOR).next().unwrap_or(name))
    });
    !fixed && !empty && !annotated
}

fn can_be_subclassed(class: Node<'_>, source: &[u8]) -> bool {
    if has_modifier(class, FINAL, source) {
        return false;
    }
    let constructors: Vec<Node<'_>> = class
        .child_by_field_name(BODY_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|member| member.kind() == CONSTRUCTOR)
        .collect();
    constructors.is_empty()
        || constructors
            .iter()
            .any(|constructor| !has_modifier(*constructor, PRIVATE, source))
}

// La clase y todas las que la encierran son públicas o protegidas; un miembro de una interfaz es
// público aunque no lo diga, y una clase anónima o local no se ve desde afuera.
fn visible_outside_package(class: Node<'_>, source: &[u8]) -> bool {
    let mut current = Some(class);
    while let Some(node) = current {
        if TYPE_DECLARATIONS.contains(&node.kind()) {
            let in_interface = node
                .parent()
                .is_some_and(|body| body.kind() == INTERFACE_BODY || body.kind() == ANNOTATION_BODY);
            let visible =
                has_modifier(node, PUBLIC, source) || has_modifier(node, PROTECTED, source) || in_interface;
            if !visible {
                return false;
            }
        } else if LOCAL_SCOPES.contains(&node.kind()) {
            return false;
        }
        current = node.parent();
    }
    true
}

// Calca VisibilityModifier con sun_checks: todo campo que no es privado ni static final, salvo
// serialVersionUID público y los que llevan @Rule, @ClassRule, @TempDir o @VisibleForTesting.
fn visible_fields(field: Node<'_>, source: &[u8], imports: &[String]) -> Vec<LintFinding> {
    let constant = has_modifier(field, STATIC, source) && has_modifier(field, FINAL, source);
    if constant || has_modifier(field, PRIVATE, source) || allowed_annotation(field, source, imports) {
        return Vec::new();
    }
    let public = has_modifier(field, PUBLIC, source);
    named_children(field)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
        .filter(|name| !(public && text(*name, source) == PUBLIC_MEMBER_NAME))
        .map(|name| {
            finding(
                LintKind::Java(JavaRule::VisibilityModifier),
                name,
                text(name, source),
                None,
            )
        })
        .collect()
}

fn allowed_annotation(field: Node<'_>, source: &[u8], imports: &[String]) -> bool {
    annotation_names(field, source).into_iter().any(|written| {
        VISIBLE_FIELD_ANNOTATIONS.iter().any(|canonical| {
            *canonical == written
                || (canonical.rsplit(PATH_SEPARATOR).next() == Some(written)
                    && imports.iter().any(|import| import == canonical))
        })
    })
}

// LocalFinalVariableName: las variables locales final (también la de un for mejorado) y los
// recursos de un try, que son final aunque no lo digan.
fn local_final_names(node: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let is_final = node.kind() == RESOURCE || has_modifier(node, FINAL, source);
    if !is_final {
        return Vec::new();
    }
    let names: Vec<Node<'_>> = match node.kind() {
        LOCAL => named_children(node)
            .into_iter()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
            .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
            .collect(),
        _ => node.child_by_field_name(NAME_FIELD).into_iter().collect(),
    };
    names
        .into_iter()
        .filter(|name| !is_local_final_name(text(*name, source)))
        .map(|name| {
            finding(
                LintKind::Java(JavaRule::LocalFinalVariableName),
                name,
                text(name, source),
                None,
            )
        })
        .collect()
}

fn is_local_final_name(name: &str) -> bool {
    if name == WORD_SEPARATOR.to_string() {
        return true;
    }
    let mut characters = name.chars();
    characters.next().is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| character.is_ascii_alphanumeric())
}
