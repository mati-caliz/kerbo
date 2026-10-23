use tree_sitter::Node;

use super::effects::contains_effect;
use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{BLOCK, CALL, FUNCTION_FIELD, MODULE};
use crate::lint::python::formats::is_convertible_format_call;
use crate::rules::PythonRule;

const DJANGO_TRANSLATIONS: [&str; 2] = [
    "django.utils.translation.gettext",
    "django.utils.translation.gettext_lazy",
];
const STATEMENT_SUFFIX: &str = "_statement";
const DEFINITION_SUFFIX: &str = "_definition";

// UP032: `"{}".format(x)` que se escribe `f"{x}"`. Como ruff, no avisa adentro de un `gettext` de
// Django, que acepta el formato con `.format` pero no una f-string.
pub fn format_call_report(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Option<Report> {
    let never_builtin = |_: Node<'_>| false;
    let has_effect = |node: Node<'_>| contains_effect(node, source, &never_builtin);
    let convertible = is_convertible_format_call(call, source, &has_effect);
    (convertible && !is_inside_translation(model, source, call))
        .then(|| Report::at(PythonRule::UP032, call, ""))
}

// `current_expressions` de ruff: las expresiones que contienen a la llamada, hasta la sentencia.
fn is_inside_translation(model: &Model<'_>, source: &[u8], call: Node<'_>) -> bool {
    let mut current = call.parent();
    while let Some(node) = current.filter(|node| !is_statement_boundary(*node)) {
        let is_translation = node.kind() == CALL
            && node
                .child_by_field_name(FUNCTION_FIELD)
                .and_then(|function| qualified_name(model, function, source))
                .is_some_and(|name| DJANGO_TRANSLATIONS.contains(&name.as_str()));
        if is_translation {
            return true;
        }
        current = node.parent();
    }
    false
}

fn is_statement_boundary(node: Node<'_>) -> bool {
    let kind = node.kind();
    kind == BLOCK || kind == MODULE || kind.ends_with(STATEMENT_SUFFIX) || kind.ends_with(DEFINITION_SUFFIX)
}
