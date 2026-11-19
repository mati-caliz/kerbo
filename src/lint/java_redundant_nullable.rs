use tree_sitter::Node;

use super::java::{
    ANNOTATION, ARRAY_CREATION, BODY_FIELD, CLASS, CLASS_BODY, ENUM_CONSTANT, FINAL, IDENTIFIER,
    MARKER_ANNOTATION, METHOD, NAME_FIELD, OBJECT_CREATION, PRIVATE, RECORD, STATIC, TYPE_FIELD,
    VARIABLE_DECLARATOR, has_modifier, modifiers, text, unparenthesized,
};
use super::java_nullness_annotations::is_recognized;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

const NULLABLE_ANNOTATIONS: &[&str] = &["Nullable", "CheckForNull"];
const CLOSING_MODIFIERS: &[&str] = &[STATIC, PRIVATE, FINAL];
const CLOSED_OWNERS: &[&str] = &[OBJECT_CREATION, ENUM_CONSTANT, RECORD];
const QUALIFIER_SEPARATOR: char = '.';
const ARRAY_TYPE: &str = "array_type";
const BOXED_VOID: &str = "Void";
const LAMBDA: &str = "lambda_expression";
const RETURN: &str = "return_statement";
const LOCAL_DECLARATION: &str = "local_variable_declaration";
const ASSIGNMENT: &str = "assignment_expression";
const INVOCATION: &str = "method_invocation";
const CAST: &str = "cast_expression";
const TERNARY: &str = "ternary_expression";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const LEFT_FIELD: &str = "left";
const VALUE_FIELD: &str = "value";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const NEVER_NULL_KINDS: &[&str] = &[
    OBJECT_CREATION,
    ARRAY_CREATION,
    "this",
    "class_literal",
    "binary_expression",
    "unary_expression",
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
];
const LONGEST_VARIABLE_TRAIL: usize = 8;

// Calca "@Nullable method 'x' always returns a non-null value" de DataFlowIssue: un método que
// nadie puede redefinir, anotado `@Nullable` o `@CheckForNull`, cuyos `return` devuelven todos algo
// que nunca es null. IntelliJ lo decide con el flujo entero; acá sólo cuenta lo que se ve sin
// seguirlo: literales, `new`, `this`, operaciones, y una local asignada una sola vez con algo así.
pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    nodes
        .iter()
        .filter(|node| node.kind() == METHOD)
        .filter_map(|method| redundant_annotation(*method, source))
        .collect()
}

fn redundant_annotation(method: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let annotation_name = nullable_annotation_name(method, source)?;
    let returned_type = method.child_by_field_name(TYPE_FIELD)?;
    if returned_type.kind() == ARRAY_TYPE
        || text(returned_type, source) == BOXED_VOID
        || can_be_overridden(method)
    {
        return None;
    }
    let code = own_code(method.child_by_field_name(BODY_FIELD)?);
    if code.iter().any(|node| node.kind() == LAMBDA) {
        return None;
    }
    let body = Body { code: &code, source };
    let mut returned_values = code
        .iter()
        .filter(|node| node.kind() == RETURN)
        .filter_map(|statement| returned_value(*statement))
        .peekable();
    returned_values.peek()?;
    if !returned_values.all(|value| body.is_never_null(value, 0)) {
        return None;
    }
    Some(finding(
        LintKind::Java(JavaRule::RedundantNullable),
        annotation_name,
        text(method.child_by_field_name(NAME_FIELD)?, source),
        None,
    ))
}

fn nullable_annotation_name<'tree>(method: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    modifiers(method)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .chain(named_children(method))
        .filter(|child| child.kind() == MARKER_ANNOTATION || child.kind() == ANNOTATION)
        .filter_map(|annotation| annotation.child_by_field_name(NAME_FIELD))
        .find(|name| {
            let simple = text(*name, source).rsplit(QUALIFIER_SEPARATOR).next();
            simple.is_some_and(|simple| NULLABLE_ANNOTATIONS.contains(&simple))
                && is_recognized(*name, source)
        })
}

// Como `PsiUtil.canBeOverridden`: no se puede si el método es estático, privado o `final`, o si su
// clase es anónima (también el cuerpo de una constante de enum), un record o `final`.
fn can_be_overridden(method: Node<'_>) -> bool {
    if CLOSING_MODIFIERS
        .iter()
        .any(|modifier| has_modifier(method, modifier))
    {
        return false;
    }
    let owner = method
        .parent()
        .filter(|body| body.kind() == CLASS_BODY)
        .and_then(|body| body.parent());
    !owner.is_some_and(|owner| {
        CLOSED_OWNERS.contains(&owner.kind()) || (owner.kind() == CLASS && has_modifier(owner, FINAL))
    })
}

// Los nodos del cuerpo sin entrar a las clases declaradas adentro: sus `return` son de otro método.
fn own_code(body: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![body];
    while let Some(node) = pending.pop() {
        if node.kind() == CLASS_BODY {
            continue;
        }
        found.push(node);
        pending.extend(
            (0..node.child_count())
                .rev()
                .filter_map(|index| node.child(index)),
        );
    }
    found
}

fn returned_value(statement: Node<'_>) -> Option<Node<'_>> {
    named_children(statement)
        .into_iter()
        .find(|child| !COMMENTS.contains(&child.kind()))
}

struct Body<'code, 'tree> {
    code: &'code [Node<'tree>],
    source: &'code [u8],
}

impl<'tree> Body<'_, 'tree> {
    fn is_never_null(&self, expression: Node<'tree>, followed: usize) -> bool {
        let expression = unparenthesized(expression);
        match expression.kind() {
            kind if NEVER_NULL_KINDS.contains(&kind) => true,
            CAST => expression
                .child_by_field_name(VALUE_FIELD)
                .is_some_and(|value| self.is_never_null(value, followed)),
            TERNARY => [CONSEQUENCE_FIELD, ALTERNATIVE_FIELD].iter().all(|branch| {
                expression
                    .child_by_field_name(branch)
                    .is_some_and(|value| self.is_never_null(value, followed))
            }),
            IDENTIFIER if followed < LONGEST_VARIABLE_TRAIL => self
                .only_value_of(expression)
                .is_some_and(|value| self.is_never_null(value, followed + 1)),
            _ => false,
        }
    }

    // Lo único que se le asigna a la variable: una local declarada una sola vez en un bloque que
    // encierra al uso, con valor inicial y sin ninguna asignación después.
    fn only_value_of(&self, variable: Node<'tree>) -> Option<Node<'tree>> {
        let name = text(variable, self.source);
        let mut declarators = self
            .code
            .iter()
            .filter(|node| node.kind() == IDENTIFIER && text(**node, self.source) == name)
            .filter_map(|identifier| declared_by(*identifier));
        let declarator = declarators.next()?;
        let declaration = declarator
            .parent()
            .filter(|parent| parent.kind() == LOCAL_DECLARATION)?;
        let scope = declaration.parent()?;
        let is_reassigned = self.code.iter().any(|node| {
            node.kind() == ASSIGNMENT
                && node
                    .child_by_field_name(LEFT_FIELD)
                    .is_some_and(|left| left.kind() == IDENTIFIER && text(left, self.source) == name)
        });
        let encloses_use = std::iter::successors(variable.parent(), Node::parent)
            .any(|ancestor| ancestor.id() == scope.id());
        if declarators.next().is_some()
            || declarator.kind() != VARIABLE_DECLARATOR
            || is_reassigned
            || !encloses_use
        {
            return None;
        }
        declarator.child_by_field_name(VALUE_FIELD)
    }
}

// El nodo que declara una variable con ese identificador como nombre (declarador, parámetro de un
// `catch`, variable de un `for`, patrón): el nombre de un método llamado no declara nada.
fn declared_by(identifier: Node<'_>) -> Option<Node<'_>> {
    identifier.parent().filter(|parent| {
        parent.kind() != INVOCATION
            && parent
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| name.id() == identifier.id())
    })
}
