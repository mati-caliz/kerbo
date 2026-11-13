use tree_sitter::Node;

use super::java::{
    ANNOTATION, CONSTRUCTOR, ENUM_CONSTANT, MARKER_ANNOTATION, METHOD, NAME_FIELD, PROGRAM,
    VARIABLE_DECLARATOR, is_type_declaration, modifiers, text,
};
use super::java_javadoc_checks::{method_tag_findings, type_tag_findings};
use super::java_javadoc_scope::{Scope, declared_scope, is_in_code_block, surrounding_scope};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

const BLOCK_COMMENT: &str = "block_comment";
const LINE_COMMENT: &str = "line_comment";
const JAVADOC_OPENER: &str = "/**";
const EMPTY_BLOCK_COMMENT: &str = "/**/";
pub const COMPACT_CONSTRUCTOR: &str = "compact_constructor_declaration";
pub const ANNOTATION_ELEMENT: &str = "annotation_type_element_declaration";
const FIELD: &str = "field_declaration";
const CONSTANT: &str = "constant_declaration";
const PACKAGE: &str = "package_declaration";
const MODULE: &str = "module_declaration";
const SERIAL_VERSION: &str = "serialVersionUID";
const OVERRIDE: &str = "Override";
pub const METHOD_LIKE: [&str; 4] = [METHOD, CONSTRUCTOR, COMPACT_CONSTRUCTOR, ANNOTATION_ELEMENT];
const VARIABLES: [&str; 3] = [FIELD, CONSTANT, ENUM_CONSTANT];
const DOCUMENTABLE_MEMBERS: [&str; 9] = [
    METHOD,
    CONSTRUCTOR,
    COMPACT_CONSTRUCTOR,
    ANNOTATION_ELEMENT,
    FIELD,
    CONSTANT,
    ENUM_CONSTANT,
    PACKAGE,
    MODULE,
];

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let is_package_info = nodes.first().is_some_and(|root| is_package_info(*root));
    for node in nodes {
        let kind = node.kind();
        if kind == BLOCK_COMMENT && !is_package_info {
            findings.extend(misplaced_javadoc(*node, source));
        } else if VARIABLES.contains(&kind) {
            findings.extend(undocumented_variable(*node, source));
        } else if METHOD_LIKE.contains(&kind) {
            findings.extend(undocumented_method(*node, source));
            findings.extend(method_tag_findings(*node, source));
        } else if is_type_declaration(*node) {
            findings.extend(type_tag_findings(*node, source));
        }
    }
    findings
}

fn is_package_info(root: Node<'_>) -> bool {
    let declarations = named_children(root);
    root.kind() == PROGRAM
        && declarations.iter().any(|child| child.kind() == PACKAGE)
        && !declarations.iter().any(|child| is_type_declaration(*child))
}

fn is_comment(node: Node<'_>) -> bool {
    [BLOCK_COMMENT, LINE_COMMENT].contains(&node.kind())
}

pub fn is_javadoc(comment: Node<'_>, source: &[u8]) -> bool {
    let written = text(comment, source);
    comment.kind() == BLOCK_COMMENT && written.starts_with(JAVADOC_OPENER) && written != EMPTY_BLOCK_COMMENT
}

// Como Checkstyle 14, el Javadoc de una declaración es el más cercano de los comentarios pegados a
// su primer token, aunque haya líneas en blanco o comentarios comunes en el medio.
pub fn attached_javadoc<'tree>(declaration: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    std::iter::successors(declaration.prev_sibling(), Node::prev_sibling)
        .take_while(|sibling| is_comment(*sibling))
        .find(|comment| is_javadoc(*comment, source))
}

pub fn written_annotations<'source>(declaration: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    modifiers(declaration)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|child| [ANNOTATION, MARKER_ANNOTATION].contains(&child.kind()))
        .filter_map(|annotation| annotation.child_by_field_name(NAME_FIELD))
        .map(|name| text(name, source))
        .collect()
}

pub fn overrides(declaration: Node<'_>, source: &[u8]) -> bool {
    declaration.kind() != ANNOTATION_ELEMENT && written_annotations(declaration, source).contains(&OVERRIDE)
}

fn declaration_name<'source>(declaration: Node<'_>, source: &'source [u8]) -> &'source str {
    declaration
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source))
        .unwrap_or_default()
}

// Un Javadoc vale si lo que le sigue, salteando comentarios comunes, es algo que se documenta; si
// antes aparece otro Javadoc, el que vale es ese.
fn misplaced_javadoc(comment: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    if !is_javadoc(comment, source) {
        return None;
    }
    let next = std::iter::successors(comment.next_sibling(), Node::next_sibling)
        .find(|sibling| !is_comment(*sibling) || is_javadoc(*sibling, source));
    let documents = next.is_some_and(|next| {
        !is_comment(next) && (is_type_declaration(next) || DOCUMENTABLE_MEMBERS.contains(&next.kind()))
    });
    (!documents).then(|| {
        finding(
            LintKind::Java(JavaRule::InvalidJavadocPosition),
            comment,
            "",
            None,
        )
    })
}

fn declared_names<'source>(declaration: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    if declaration.kind() == ENUM_CONSTANT {
        return vec![declaration_name(declaration, source)];
    }
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .map(|declarator| declaration_name(declarator, source))
        .collect()
}

// JavadocVariable: los campos y las constantes de enum sin Javadoc, fuera de los métodos y los
// bloques; serialVersionUID queda afuera. Checkstyle avisa una vez por variable en la misma posición
// y el arnés de paridad las junta, así que va un aviso por declaración.
fn undocumented_variable(declaration: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    if is_in_code_block(declaration) || attached_javadoc(declaration, source).is_some() {
        return None;
    }
    let name = declared_names(declaration, source)
        .into_iter()
        .find(|name| *name != SERIAL_VERSION)?;
    Some(finding(
        LintKind::Java(JavaRule::JavadocVariable),
        declaration,
        name,
        None,
    ))
}

// MissingJavadocMethod con el alcance por defecto: lo público de tipos públicos, sin @Override.
fn undocumented_method(declaration: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let visible =
        declared_scope(declaration) == Scope::Public && surrounding_scope(declaration) == Scope::Public;
    let missing =
        visible && attached_javadoc(declaration, source).is_none() && !overrides(declaration, source);
    missing.then(|| {
        finding(
            LintKind::Java(JavaRule::MissingJavadocMethod),
            declaration,
            declaration_name(declaration, source),
            None,
        )
    })
}
