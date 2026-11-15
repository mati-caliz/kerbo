use tree_sitter::Node;

use super::java::{IDENTIFIER, NAME_FIELD, TYPE_FIELD, VARIABLE_DECLARATOR, text, unparenthesized};
use super::java_math_api::is_builder;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const STRING_REPEAT_RELEASE: u32 = 11;
const FOR: &str = "for_statement";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const BLOCK: &str = "block";
const INVOCATION: &str = "method_invocation";
const BINARY: &str = "binary_expression";
const UPDATE: &str = "update_expression";
const ASSIGNMENT: &str = "assignment_expression";
const OBJECT_CREATION: &str = "object_creation_expression";
const FOR_KEYWORD: &str = "for";
const INIT_FIELD: &str = "init";
const CONDITION_FIELD: &str = "condition";
const UPDATE_FIELD: &str = "update";
const BODY_FIELD: &str = "body";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const COMMENT_SUFFIX: &str = "comment";
const INT: &str = "int";
const APPEND: &str = "append";
const INCREMENTS: [&str; 2] = ["++", "+="];
const DECREMENTS: [&str; 2] = ["--", "-="];
// `CountingLoop` de IntelliJ: el contador va hacia el límite con `<`, `<=` o `!=` (o al revés si baja).
const UPWARD_BOUNDS: [&str; 3] = ["<", "<=", "!="];
const DOWNWARD_BOUNDS: [&str; 3] = [">", ">=", "!="];
const ONE: &str = "1";
const EFFECTS: [&str; 4] = [INVOCATION, ASSIGNMENT, UPDATE, OBJECT_CREATION];

// `StringRepeatCanBeUsed` de IntelliJ, la forma del bucle: un `for` que cuenta con un `int` y en
// cada vuelta sólo agrega lo mismo a un builder (`for (int i = 0; i < n; i++) linea.append("-");`),
// que `"-".repeat(n)` hace de una vez. Desde Java 11; lo agregado no puede usar el contador ni tener
// efectos.
pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if release.is_some_and(|release| release < STRING_REPEAT_RELEASE) {
        return Vec::new();
    }
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == FOR)
        .filter(|loop_statement| repeats_an_append(*loop_statement, source))
        .filter_map(|loop_statement| {
            let keyword = loop_statement
                .child(0)
                .filter(|keyword| keyword.kind() == FOR_KEYWORD)?;
            Some(finding(
                LintKind::Java(JavaRule::StringRepeatCanBeUsed),
                keyword,
                FOR_KEYWORD,
                None,
            ))
        })
        .collect()
}

fn repeats_an_append(loop_statement: Node<'_>, source: &[u8]) -> bool {
    let Some(counter) = counter(loop_statement, source) else {
        return false;
    };
    let Some(call) = single_append(loop_statement, source) else {
        return false;
    };
    let mentions_counter = |node: Node<'_>| {
        descendants(node)
            .into_iter()
            .any(|part| part.kind() == IDENTIFIER && text(part, source) == counter)
    };
    let argument = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .and_then(|arguments| named_children(arguments).into_iter().next());
    let body = loop_statement.child_by_field_name(BODY_FIELD);
    counts_up_or_down(loop_statement, counter, source)
        && argument.is_some_and(|argument| {
            !mentions_counter(argument)
                && !descendants(argument)
                    .iter()
                    .any(|part| EFFECTS.contains(&part.kind()))
        })
        && !mentions_counter(call)
        && body.is_none_or(|body| !assigns(body, counter, source))
}

fn counter<'source>(loop_statement: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let init = loop_statement.child_by_field_name(INIT_FIELD)?;
    if init.kind() != LOCAL_VARIABLE || text(init.child_by_field_name(TYPE_FIELD)?, source) != INT {
        return None;
    }
    let [declarator] = named_children(init)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .collect::<Vec<_>>()[..]
    else {
        return None;
    };
    declarator.child_by_field_name(VALUE_FIELD)?;
    Some(text(declarator.child_by_field_name(NAME_FIELD)?, source))
}

fn counts_up_or_down(loop_statement: Node<'_>, counter: &str, source: &[u8]) -> bool {
    let Some(ascending) = direction(loop_statement, counter, source) else {
        return false;
    };
    let Some(condition) = loop_statement
        .child_by_field_name(CONDITION_FIELD)
        .map(unparenthesized)
    else {
        return false;
    };
    if condition.kind() != BINARY {
        return false;
    }
    let operator = condition
        .child_by_field_name(OPERATOR_FIELD)
        .map_or("", |operator| text(operator, source));
    let is_counter = |field: &str| {
        condition
            .child_by_field_name(field)
            .is_some_and(|side| side.kind() == IDENTIFIER && text(side, source) == counter)
    };
    let (counter_side, toward) = if is_counter(LEFT_FIELD) {
        (true, ascending)
    } else if is_counter(RIGHT_FIELD) {
        (true, !ascending)
    } else {
        (false, ascending)
    };
    let allowed = if toward { &UPWARD_BOUNDS } else { &DOWNWARD_BOUNDS };
    counter_side && allowed.contains(&operator)
}

fn direction(loop_statement: Node<'_>, counter: &str, source: &[u8]) -> Option<bool> {
    let update = loop_statement.child_by_field_name(UPDATE_FIELD)?;
    let written: String = text(update, source)
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let rest = written.strip_prefix(counter).or_else(|| {
        written
            .strip_suffix(counter)
            .filter(|operator| INCREMENTS.contains(operator) || DECREMENTS.contains(operator))
    })?;
    let step = rest.strip_suffix(ONE).unwrap_or(rest);
    let compound = rest.len() != step.len();
    if INCREMENTS.contains(&step) && (compound || step == INCREMENTS[0]) {
        Some(true)
    } else if DECREMENTS.contains(&step) && (compound || step == DECREMENTS[0]) {
        Some(false)
    } else {
        None
    }
}

fn single_append<'tree>(loop_statement: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let mut body = loop_statement.child_by_field_name(BODY_FIELD)?;
    if body.kind() == BLOCK {
        let [only] = named_children(body)
            .into_iter()
            .filter(|statement| !statement.kind().ends_with(COMMENT_SUFFIX))
            .collect::<Vec<_>>()[..]
        else {
            return None;
        };
        body = only;
    }
    if body.kind() != EXPRESSION_STATEMENT {
        return None;
    }
    let call = named_children(body).into_iter().next()?;
    let is_append = call.kind() == INVOCATION
        && call
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == APPEND)
        && call
            .child_by_field_name(ARGUMENTS_FIELD)
            .is_some_and(|arguments| named_children(arguments).len() == 1);
    let builder = call.child_by_field_name(OBJECT_FIELD)?;
    (is_append && builder.kind() == IDENTIFIER && is_builder(builder, source)).then_some(call)
}

fn assigns(body: Node<'_>, counter: &str, source: &[u8]) -> bool {
    descendants(body).into_iter().any(|node| {
        let target = match node.kind() {
            ASSIGNMENT => node.child_by_field_name(LEFT_FIELD),
            UPDATE => named_children(node).into_iter().next(),
            _ => None,
        };
        target.is_some_and(|target| target.kind() == IDENTIFIER && text(target, source) == counter)
    })
}
