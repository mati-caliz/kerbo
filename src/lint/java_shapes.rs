use tree_sitter::Node;

use super::java::{
    ABSTRACT, CLASS, CONSTRUCTOR, INTERFACE, METHOD, NAME_FIELD, PARAMETERS_FIELD, PRIVATE, PROTECTED,
    PUBLIC, STATIC, TYPE_FIELD, children, modifiers, text,
};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

const IMPORT: &str = "import_declaration";
const ILLEGAL_PACKAGE: &str = "sun";
const PACKAGE_SEPARATOR: &str = ".";
const IMPORT_KEYWORD: &str = "import";
const STATIC_KEYWORD: &str = "static";
const JLS_ORDER: &[&str] = &[
    "public",
    "protected",
    "private",
    "abstract",
    "default",
    "static",
    "sealed",
    "non-sealed",
    "final",
    "transient",
    "volatile",
    "synchronized",
    "native",
    "strictfp",
];
const ANNOTATIONS: &[&str] = &["marker_annotation", "annotation"];
const TYPE_ANNOTATION_OWNERS: &[&str] = &[
    "field_declaration",
    "local_variable_declaration",
    "formal_parameter",
    "constructor_declaration",
    "constant_declaration",
];
const VOID: &str = "void_type";
const DIMENSIONS_FIELD: &str = "dimensions";
const DECLARATIONS: &[&str] = &[
    "field_declaration",
    "local_variable_declaration",
    "constant_declaration",
];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const METHOD_CONTAINERS: &[&str] = &["class_body", "enum_body_declarations"];
const DECLARATOR: &str = "variable_declarator";
const DECLARATOR_FIELD: &str = "declarator";
const FOR: &str = "for_statement";
const LOCAL: &str = "local_variable_declaration";
const INTERFACE_BODY: &str = "interface_body";
const INTERFACE_FIELD: &str = "constant_declaration";
const EQUALS: &str = "equals";
const HASH_CODE: &str = "hashCode";
const OBJECT_TYPES: &[&str] = &["Object", "java.lang.Object"];
const SUPERCLASS_FIELD: &str = "superclass";
const BODY_FIELD: &str = "body";
const FIELD: &str = "field_declaration";

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let mut for_initializers: Vec<usize> = Vec::new();
    for &node in nodes {
        let kind = node.kind();
        if kind == IMPORT && is_illegal_import(node, source) {
            findings.push(finding(LintKind::Java(JavaRule::IllegalImport), node, "", None));
        }
        if kind == super::java::MODIFIERS {
            findings.extend(modifier_out_of_order(node, source));
        }
        findings.extend(c_style_brackets(node));
        if kind == FOR {
            for_initializers.extend(
                named_children(node)
                    .into_iter()
                    .filter(|child| child.kind() == LOCAL)
                    .map(|child| child.id()),
            );
        }
        if DECLARATIONS.contains(&kind) && !for_initializers.contains(&node.id()) {
            findings.extend(comma_separated(node));
        }
        findings.extend(declarations_on_one_line(node));
        if kind == INTERFACE && only_constants(node) {
            findings.push(finding(LintKind::Java(JavaRule::InterfaceIsType), node, "", None));
        }
        if METHOD_CONTAINERS.contains(&kind) {
            findings.extend(equals_without_hash_code(node, source));
        }
        if kind == CLASS {
            findings.extend(utility_constructor(node, source));
        }
    }
    findings
}

fn is_illegal_import(import: Node<'_>, source: &[u8]) -> bool {
    children(import)
        .into_iter()
        .filter(Node::is_named)
        .map(|child| text(child, source))
        .find(|written| *written != IMPORT_KEYWORD && *written != STATIC_KEYWORD)
        .is_some_and(|path| {
            path == ILLEGAL_PACKAGE || path.starts_with(&format!("{ILLEGAL_PACKAGE}{PACKAGE_SEPARATOR}"))
        })
}

// Calca ModifierOrder: las anotaciones del principio se saltean, y una que aparece después de un
// modificador es un error salvo que vaya sobre el tipo (variables, parámetros, constructores y
// métodos que devuelven algo). Los modificadores tienen que seguir el orden de la JLS.
fn modifier_out_of_order(modifiers: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let all: Vec<Node<'_>> = children(modifiers)
        .into_iter()
        .filter(|child| !COMMENTS.contains(&child.kind()))
        .collect();
    let mut remaining = all
        .iter()
        .copied()
        .skip_while(|modifier| ANNOTATIONS.contains(&modifier.kind()))
        .peekable();
    remaining.peek()?;
    let mut position = 0;
    for modifier in remaining {
        if ANNOTATIONS.contains(&modifier.kind()) {
            return (!annotates_type(modifiers)).then(|| {
                finding(
                    LintKind::Java(JavaRule::ModifierOrder),
                    modifier,
                    text(modifier, source),
                    None,
                )
            });
        }
        let written = text(modifier, source);
        while JLS_ORDER
            .get(position)
            .is_some_and(|expected| *expected != written)
        {
            position += 1;
        }
        if position >= JLS_ORDER.len() {
            return Some(finding(
                LintKind::Java(JavaRule::ModifierOrder),
                modifier,
                written,
                None,
            ));
        }
    }
    None
}

fn annotates_type(modifiers: Node<'_>) -> bool {
    modifiers.parent().is_some_and(|owner| {
        TYPE_ANNOTATION_OWNERS.contains(&owner.kind())
            || (owner.kind() == METHOD
                && owner
                    .child_by_field_name(TYPE_FIELD)
                    .is_some_and(|returned| returned.kind() != VOID))
    })
}

fn c_style_brackets(node: Node<'_>) -> Option<LintFinding> {
    let owner_has_type = matches!(
        node.kind(),
        "variable_declarator" | "formal_parameter" | "method_declaration" | "enhanced_for_statement"
    );
    if !owner_has_type {
        return None;
    }
    let dimensions = node.child_by_field_name(DIMENSIONS_FIELD)?;
    Some(finding(
        LintKind::Java(JavaRule::ArrayTypeStyle),
        dimensions,
        "",
        None,
    ))
}

// Dos variables separadas por coma (salvo en el inicio de un `for`): Checkstyle marca la declaración.
fn comma_separated(declaration: Node<'_>) -> Option<LintFinding> {
    let mut cursor = declaration.walk();
    let declarators = declaration
        .children_by_field_name(DECLARATOR_FIELD, &mut cursor)
        .filter(|child| child.kind() == DECLARATOR)
        .count();
    (declarators > 1).then(|| {
        finding(
            LintKind::Java(JavaRule::MultipleVariableDeclarations),
            declaration,
            "",
            None,
        )
    })
}

// Dos declaraciones seguidas (sin contar comentarios) que arrancan en la misma línea: se marca la
// primera. Se mira desde el contenedor: pedir el hermano siguiente en tree-sitter es caro.
fn declarations_on_one_line(container: Node<'_>) -> Vec<LintFinding> {
    let members: Vec<Node<'_>> = named_children(container)
        .into_iter()
        .filter(|child| !COMMENTS.contains(&child.kind()))
        .collect();
    members
        .windows(2)
        .filter_map(|pair| match pair {
            [current, next]
                if DECLARATIONS.contains(&current.kind())
                    && DECLARATIONS.contains(&next.kind())
                    && current.start_position().row == next.start_position().row =>
            {
                Some(finding(
                    LintKind::Java(JavaRule::MultipleVariableDeclarations),
                    *current,
                    "",
                    None,
                ))
            }
            _ => None,
        })
        .collect()
}

fn only_constants(interface: Node<'_>) -> bool {
    let Some(body) = interface.child_by_field_name(BODY_FIELD) else {
        return false;
    };
    let members = named_children(body);
    let has_constants = members.iter().any(|member| member.kind() == INTERFACE_FIELD);
    has_constants && !members.iter().any(|member| member.kind() == METHOD) && body.kind() == INTERFACE_BODY
}

fn public_instance_methods<'tree>(container: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    named_children(container)
        .into_iter()
        .filter(|member| {
            member.kind() == METHOD
                && has_modifier(*member, PUBLIC, source)
                && !has_modifier(*member, STATIC, source)
                && !has_modifier(*member, ABSTRACT, source)
        })
        .collect()
}

fn parameter_count(method: Node<'_>) -> usize {
    method
        .child_by_field_name(PARAMETERS_FIELD)
        .map_or(0, |parameters| named_children(parameters).len())
}

fn is_equals(method: Node<'_>, source: &[u8]) -> bool {
    method
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source))
        == Some(EQUALS)
        && method
            .child_by_field_name(PARAMETERS_FIELD)
            .map(named_children)
            .is_some_and(|parameters| {
                parameters.len() == 1
                    && parameters
                        .first()
                        .and_then(|parameter| parameter.child_by_field_name(TYPE_FIELD))
                        .is_some_and(|declared| OBJECT_TYPES.contains(&text(declared, source)))
            })
}

fn is_hash_code(method: Node<'_>, source: &[u8]) -> bool {
    method
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source))
        == Some(HASH_CODE)
        && parameter_count(method) == 0
}

// EqualsHashCode mira cada cuerpo de clase (también enums, records y clases anónimas) y sólo los
// métodos públicos que no son static.
fn equals_without_hash_code(container: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let methods = public_instance_methods(container, source);
    let equals = methods.iter().find(|method| is_equals(**method, source));
    let hash_code = methods.iter().find(|method| is_hash_code(**method, source));
    match (equals, hash_code) {
        (Some(alone), None) | (None, Some(alone)) => {
            vec![finding(
                LintKind::Java(JavaRule::EqualsHashCode),
                *alone,
                "",
                None,
            )]
        }
        (Some(_), Some(_)) | (None, None) => Vec::new(),
    }
}

fn has_modifier(node: Node<'_>, modifier: &str, source: &[u8]) -> bool {
    modifiers(node).is_some_and(|modifiers| {
        children(modifiers)
            .into_iter()
            .any(|child| text(child, source) == modifier)
    })
}

// Calca HideUtilityClassConstructor: una clase que no hereda, sin métodos ni campos de instancia,
// con algún método o campo static no privado, y con el constructor implícito o uno que no es
// privado ni protegido (el de paquete cuenta como público). Las clases abstractas y las anidadas
// static quedan afuera.
fn utility_constructor(class: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    if class.child_by_field_name(SUPERCLASS_FIELD).is_some()
        || has_modifier(class, STATIC, source)
        || has_modifier(class, ABSTRACT, source)
    {
        return None;
    }
    let members = named_children(class.child_by_field_name(BODY_FIELD)?);
    let static_member = |member: &&Node<'_>| has_modifier(**member, STATIC, source);
    let instance_code = members
        .iter()
        .any(|member| (member.kind() == METHOD || member.kind() == FIELD) && !static_member(&member));
    let visible_static = members.iter().any(|member| {
        (member.kind() == METHOD || member.kind() == FIELD)
            && static_member(&member)
            && !has_modifier(*member, PRIVATE, source)
    });
    let constructors: Vec<&Node<'_>> = members
        .iter()
        .filter(|member| member.kind() == CONSTRUCTOR)
        .collect();
    let accessible = constructors.is_empty()
        || constructors.iter().any(|constructor| {
            !has_modifier(**constructor, PRIVATE, source) && !has_modifier(**constructor, PROTECTED, source)
        });
    (!instance_code && visible_static && accessible).then(|| {
        finding(
            LintKind::Java(JavaRule::HideUtilityClassConstructor),
            class,
            "",
            None,
        )
    })
}
