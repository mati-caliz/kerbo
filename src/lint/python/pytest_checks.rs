use tree_sitter::Node;

use super::Report;
use super::ast::{
    CALL, CLASS, COMMENT, DECORATED, FUNCTION, FUNCTION_FIELD, TUPLE, VALUE_FIELD, named, text,
    unparenthesized,
};
use super::branches::equivalent;
use crate::rules::PythonRule;

const BODY_FIELD: &str = "body";
const ARGUMENTS_FIELD: &str = "arguments";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const AS_PATTERN: &str = "as_pattern";
const FOR: &str = "for_statement";
const IF: &str = "if_statement";
const WITH: &str = "with_statement";
const DECORATOR: &str = "decorator";
const PYTEST_WARNS: &str = "pytest.warns";
const PARAMETRIZE: &str = "pytest.mark.parametrize";
const ARGVALUES: &str = "argvalues";
const COMPOUND: [&str; 5] = [IF, FOR, "while_statement", "try_statement", "match_statement"];
const DEFINITIONS: [&str; 3] = [FUNCTION, CLASS, DECORATED];
const PLACEHOLDERS: [&str; 2] = ["pass_statement", "ellipsis"];
const LIST_KINDS: [&str; 2] = ["list", TUPLE];

// Chequeos de pytest: PT031 y PT014.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            WITH => found.extend(warns_with_statements(*node, source)),
            DECORATED => found.extend(duplicate_parametrize_cases(*node, source)),
            _ => {}
        }
    }
    found
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// PT031: `with pytest.warns(...)` con más de una sentencia adentro. Como ruff, deja pasar una
// definición (para probar decoradores), un `for` vacío y un `with` con una sola sentencia simple.
fn warns_with_statements(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let warns = significant(*significant(statement).first()?)
        .into_iter()
        .filter_map(|item| significant(item).first().copied())
        .map(|value| {
            if value.kind() == AS_PATTERN {
                significant(value).first().copied().unwrap_or(value)
            } else {
                value
            }
        })
        .map(unparenthesized)
        .any(|value| {
            value.kind() == CALL
                && value
                    .child_by_field_name(FUNCTION_FIELD)
                    .is_some_and(|function| text(function, source) == PYTEST_WARNS)
        });
    let body = significant(statement.child_by_field_name(BODY_FIELD)?);
    let trivial = match body.as_slice() {
        [only] if only.kind() == FOR => only
            .child_by_field_name(BODY_FIELD)
            .is_some_and(|loop_body| significant(loop_body).iter().all(|inner| is_placeholder(*inner))),
        [only] if only.kind() == WITH => only.child_by_field_name(BODY_FIELD).is_some_and(|inner| {
            matches!(significant(inner).as_slice(), [single] if !COMPOUND.contains(&single.kind())
                && !DEFINITIONS.contains(&single.kind()) && single.kind() != WITH)
        }),
        [only] => !COMPOUND.contains(&only.kind()),
        _ => false,
    };
    (warns && !trivial).then(|| Report::at(PythonRule::PT031, statement, ""))
}
fn is_placeholder(statement: Node<'_>) -> bool {
    PLACEHOLDERS.contains(&statement.kind())
        || (statement.kind() == EXPRESSION_STATEMENT
            && significant(statement)
                .iter()
                .all(|value| PLACEHOLDERS.contains(&value.kind())))
}
// PT014: un caso repetido en `@pytest.mark.parametrize`. Avisa en el repetido.
fn duplicate_parametrize_cases(definition: Node<'_>, source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for call in significant(definition)
        .into_iter()
        .filter(|decorator| decorator.kind() == DECORATOR)
        .filter_map(|decorator| significant(decorator).first().copied())
        .filter(|call| {
            call.kind() == CALL
                && call
                    .child_by_field_name(FUNCTION_FIELD)
                    .is_some_and(|function| text(function, source) == PARAMETRIZE)
        })
    {
        let arguments = call
            .child_by_field_name(ARGUMENTS_FIELD)
            .map(significant)
            .unwrap_or_default();
        let values = arguments
            .iter()
            .find(|argument| {
                argument.kind() == "keyword_argument"
                    && argument
                        .child_by_field_name("name")
                        .is_some_and(|name| text(name, source) == ARGVALUES)
            })
            .and_then(|keyword| keyword.child_by_field_name(VALUE_FIELD))
            .or_else(|| arguments.get(1).copied())
            .map(unparenthesized);
        let Some(values) = values.filter(|values| LIST_KINDS.contains(&values.kind())) else {
            continue;
        };
        let cases = significant(values);
        for (index, case) in cases.iter().enumerate() {
            if cases
                .iter()
                .take(index)
                .any(|earlier| equivalent(*earlier, *case, source))
            {
                found.push(Report::at(PythonRule::PT014, *case, ""));
            }
        }
    }
    found
}
