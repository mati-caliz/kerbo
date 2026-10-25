use tree_sitter::{Node, Point};

use super::builtin_calls::{arguments, keyword_value};
use super::model::Model;
use super::qualified::qualified_name;
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{ATTRIBUTE, CALL, COMMENT, FUNCTION_FIELD, named};
use crate::rules::PythonRule;

const RAISES: &str = "pytest.raises";
const RAISES_KEYWORDS: [&str; 3] = ["expected_exception", "match", "check"];
const LIST_SPLAT: &str = "list_splat";
const FIXTURE: &str = "pytest.fixture";
const YIELD_FIXTURE: &str = "pytest.yield_fixture";
const MARK_PREFIX: &str = "pytest.mark.";
const USEFIXTURES: &str = "usefixtures";

// PT010: `pytest.raises()` sin la excepción esperada, ni `match` ni `check`. Avisa en la función.
pub fn raises_without_exception(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Option<Report> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    if qualified_name(model, function, source).as_deref() != Some(RAISES) {
        return None;
    }
    let given = arguments(call);
    let has_positional = given
        .positional
        .first()
        .is_some_and(|first| first.kind() != LIST_SPLAT);
    let has_keyword = RAISES_KEYWORDS
        .iter()
        .any(|keyword| keyword_value(&given, keyword, source).is_some());
    (!has_positional && !has_keyword).then(|| Report::at(PythonRule::PT010, function, ""))
}

// Los decoradores de una función: PT020 (`@pytest.yield_fixture`), PT025 (`@pytest.mark.usefixtures`
// sobre un fixture, donde no hace nada) y PT026 (`@pytest.mark.usefixtures` sin argumentos). Avisan en
// el decorador.
pub fn pytest_decorator_reports(model: &Model<'_>, source: &[u8], decorators: &[Node<'_>]) -> Vec<Report> {
    let callee = |decorator: Node<'_>| decorated_callee(decorator_expression(decorator)?, model, source);
    let mut reports = Vec::new();
    let fixture = decorators
        .iter()
        .find(|decorator| callee(**decorator).is_some_and(|name| name == FIXTURE || name == YIELD_FIXTURE));
    if let Some(fixture) = fixture
        && callee(*fixture).is_some_and(|name| name == YIELD_FIXTURE)
    {
        reports.push(Report::at(PythonRule::PT020, *fixture, ""));
    }
    for decorator in decorators {
        let is_usefixtures = callee(*decorator)
            .and_then(|name| name.strip_prefix(MARK_PREFIX).map(|marker| marker == USEFIXTURES))
            .unwrap_or(false);
        if !is_usefixtures {
            continue;
        }
        if fixture.is_some() {
            let fix = full_lines_deletion(*decorator, source);
            reports.push(Report::at(PythonRule::PT025, *decorator, "").with_fix(Some(fix)));
        }
        if decorator_expression(*decorator).is_some_and(is_empty_mark) {
            let fix = Fix::replace(*decorator, "", source).marked_unsafe();
            reports.push(Report::at(PythonRule::PT026, *decorator, "").with_fix(Some(fix)));
        }
    }
    reports
}

// PT025 borra las líneas enteras del decorador.
fn full_lines_deletion(decorator: Node<'_>, source: &[u8]) -> Fix {
    let start = Point {
        row: decorator.start_position().row,
        column: 0,
    };
    let end = Point {
        row: decorator.end_position().row.saturating_add(1),
        column: 0,
    };
    Fix::replace_range(start, end, "", source)
}

fn decorator_expression(decorator: Node<'_>) -> Option<Node<'_>> {
    named(decorator).into_iter().find(|child| child.kind() != COMMENT)
}

// El `map_callable` de ruff: de `@x(...)` mira `x`.
fn decorated_callee(expression: Node<'_>, model: &Model<'_>, source: &[u8]) -> Option<String> {
    let callable = if expression.kind() == CALL {
        expression.child_by_field_name(FUNCTION_FIELD)?
    } else {
        expression
    };
    qualified_name(model, callable, source)
}

// `@pytest.mark.usefixtures` o `@pytest.mark.usefixtures()`; un nombre importado suelto no cuenta.
fn is_empty_mark(expression: Node<'_>) -> bool {
    match expression.kind() {
        ATTRIBUTE => true,
        CALL => {
            let given = arguments(expression);
            given.positional.is_empty() && given.keywords.is_empty()
        }
        _ => false,
    }
}
