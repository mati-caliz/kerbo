use std::collections::HashSet;

use tree_sitter::Node;

use super::Report;
use crate::rules::PythonRule;

const FUNCTION: &str = "function_definition";
const CLASS: &str = "class_definition";
const ASSIGNMENT: &str = "assignment";
const PARAMETERS: &str = "parameters";
const IDENTIFIER: &str = "identifier";
const ALIASED_IMPORT: &str = "aliased_import";
const IMPORT_FROM: &str = "import_from_statement";
const DOTTED_NAME: &str = "dotted_name";
const FOR: &str = "for_statement";
const NAME_FIELD: &str = "name";
const LEFT_FIELD: &str = "left";
const ALIAS_FIELD: &str = "alias";
const MODULE_NAME_FIELD: &str = "module_name";
const OPEN_PARENTHESIS: char = '(';
// Los arreglos que escriben una llamada a un builtin: ruff no los ofrece si el nombre está tapado
// (`has_builtin_binding`).
const FIXES_THAT_CALL_BUILTINS: &[(PythonRule, &[&str])] = &[
    (PythonRule::SIM208, &["bool"]),
    (PythonRule::SIM210, &["bool"]),
    (PythonRule::SIM103, &["bool"]),
    (PythonRule::FURB136, &["min", "max"]),
    (PythonRule::PLR1730, &["min", "max"]),
];

// Mira todo el archivo y no el alcance de cada arreglo: un nombre tapado en otra función también deja el
// arreglo afuera, que es el lado seguro.
pub fn drop_fixes_with_shadowed_builtins(reports: &mut [Report], nodes: &[Node<'_>], source: &[u8]) {
    let is_sensitive = |rule: PythonRule| {
        FIXES_THAT_CALL_BUILTINS
            .iter()
            .any(|(sensitive, _)| *sensitive == rule)
    };
    if !reports
        .iter()
        .any(|report| is_sensitive(report.rule) && report.fix.is_some())
    {
        return;
    }
    let bound = bound_names(nodes, source);
    for report in reports.iter_mut() {
        let written = report.fix.as_ref().map_or_else(String::new, |fix| {
            fix.edits.iter().map(|edit| edit.replacement.as_str()).collect()
        });
        if writes_shadowed_builtin(report.rule, &written, &bound) {
            report.fix = None;
        }
    }
}

fn writes_shadowed_builtin(rule: PythonRule, written: &str, bound: &HashSet<&str>) -> bool {
    FIXES_THAT_CALL_BUILTINS
        .iter()
        .filter(|(sensitive, _)| *sensitive == rule)
        .flat_map(|(_, names)| names.iter())
        .any(|name| bound.contains(name) && written.contains(&format!("{name}{OPEN_PARENTHESIS}")))
}

pub fn bound_names<'source>(nodes: &[Node<'_>], source: &'source [u8]) -> HashSet<&'source str> {
    let name_of = |node: Node<'_>| node.utf8_text(source).ok();
    let mut bound = HashSet::new();
    for node in nodes {
        let target = match node.kind() {
            FUNCTION | CLASS => node.child_by_field_name(NAME_FIELD),
            ASSIGNMENT | FOR => node.child_by_field_name(LEFT_FIELD),
            ALIASED_IMPORT => node.child_by_field_name(ALIAS_FIELD),
            DOTTED_NAME if is_imported_name(*node) => Some(*node),
            IDENTIFIER if node.parent().is_some_and(|parent| parent.kind() == PARAMETERS) => Some(*node),
            _ => None,
        };
        bound.extend(target.and_then(name_of));
    }
    bound
}

fn is_imported_name(name: Node<'_>) -> bool {
    name.parent().is_some_and(|parent| {
        parent.kind() == IMPORT_FROM
            && parent
                .child_by_field_name(MODULE_NAME_FIELD)
                .is_none_or(|module| module != name)
    })
}
