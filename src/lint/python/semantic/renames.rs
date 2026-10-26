use tree_sitter::Point;

use super::model::{Binding, BindingKind, EXPLICIT_EXPORT, Model, Resolution, ScopeId, ScopeKind};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::ast::{is_keyword, text};
use crate::lint::python::builtins::is_builtin;
use crate::rules::PythonRule;

const ALIASED_IMPORT: &str = "aliased_import";
const COLLECTIONS_ABC_SET: &str = "collections.abc.Set";
const ABSTRACT_SET: &str = "AbstractSet";
const PRIVATE_PREFIX: char = '_';
const LINE_FEED: u8 = b'\n';

// PYI025: `from collections.abc import Set` sin llamarlo `AbstractSet`. Sale de los bindings, como en
// los alcances diferidos de ruff; el arreglo renombra el import y todos sus usos. Como el
// `determine_applicability` de ruff, en el módulo sólo es seguro en un stub y si no se reexporta
// (`Set as Set` o en `__all__`).
pub fn unaliased_set_imports(
    model: &Model<'_>,
    source: &[u8],
    stub: bool,
    exports: &[String],
) -> Vec<Report> {
    model
        .bindings
        .iter()
        .filter(|binding| binding.kind == BindingKind::FromImport && binding.qualified == COLLECTIONS_ABC_SET)
        .filter_map(|binding| {
            let node = binding.site.node?;
            let name = text(node, source);
            if name == ABSTRACT_SET || name.starts_with(PRIVATE_PREFIX) {
                return None;
            }
            let reported = node
                .parent()
                .filter(|parent| parent.kind() == ALIASED_IMPORT)
                .unwrap_or(node);
            let available = !matches!(
                model.lookup_from(binding.scope, ABSTRACT_SET),
                Resolution::Binding(_)
            );
            let reexported = binding.has(EXPLICIT_EXPORT) || exports.iter().any(|export| export == name);
            let in_module = model.scope_of(binding.scope).kind == ScopeKind::Module;
            let renaming = Renaming {
                scope: binding.scope,
                name,
                target: ABSTRACT_SET,
            };
            let fix = available
                .then(|| rename(model, source, &renaming))
                .flatten()
                .map(|fix| {
                    if !in_module || (stub && !reexported) {
                        fix
                    } else {
                        fix.marked_unsafe()
                    }
                });
            Some(Report::at(PythonRule::PYI025, reported, "").with_fix(fix))
        })
        .collect()
}

struct Renaming<'name> {
    scope: ScopeId,
    name: &'name str,
    target: &'name str,
}

// UP049: el arreglo saca los guiones bajos del parámetro de tipo y de todos sus usos, salvo que el
// nombre nuevo tape algo (una palabra clave, un builtin u otro nombre visible) o que un uso no esté
// escrito tal cual (dentro de un string, por ejemplo).
pub fn attach_private_type_parameter_fixes(model: &Model<'_>, source: &[u8], reports: &mut [Report]) {
    for report in reports
        .iter_mut()
        .filter(|report| report.rule == PythonRule::UP049 && report.fix.is_none())
    {
        let Some(binding) = model
            .bindings
            .iter()
            .find(|binding| binding.kind == BindingKind::TypeParameter && binding.site.at == report.start)
        else {
            continue;
        };
        report.fix = private_type_parameter_fix(model, source, binding);
    }
}

fn private_type_parameter_fix(model: &Model<'_>, source: &[u8], binding: &Binding<'_>) -> Option<Fix> {
    let old_name = text(binding.site.node?, source);
    let new_name = old_name.trim_start_matches(PRIVATE_PREFIX);
    let shadows = is_keyword(new_name)
        || is_builtin(new_name)
        || std::iter::once(binding.scope)
            .chain(binding.references.iter().map(|reference| reference.scope))
            .any(|scope| matches!(model.lookup_from(scope, new_name), Resolution::Binding(_)));
    if shadows || !is_identifier(new_name) {
        return None;
    }
    let written_as_is = binding
        .references
        .iter()
        .all(|reference| text_at(source, reference.at, old_name.len()) == Some(old_name));
    if !written_as_is {
        return None;
    }
    let isolation = binding.site.statement.map(|statement| statement.id());
    let renaming = Renaming {
        scope: binding.scope,
        name: old_name,
        target: new_name,
    };
    rename(model, source, &renaming).map(|fix| fix.isolated(isolation))
}

// El `Renamer::rename` de ruff, sin los `global`/`nonlocal`: cada binding del nombre en el alcance y
// todas sus referencias. Un import sin alias suma ` as nuevo` y uno con alias cambia el alias.
fn rename(model: &Model<'_>, source: &[u8], renaming: &Renaming<'_>) -> Option<Fix> {
    let Renaming { scope, name, target } = *renaming;
    let mut edits: Vec<TextEdit> = Vec::new();
    for binding in model
        .bindings
        .iter()
        .filter(|binding| binding.scope == scope)
        .filter(|binding| binding.site.node.is_some_and(|node| text(node, source) == name))
    {
        let node = binding.site.node?;
        let replacement = match binding.kind {
            BindingKind::Import | BindingKind::FromImport
                if node.parent().is_none_or(|parent| parent.kind() != ALIASED_IMPORT) =>
            {
                format!("{name} as {target}")
            }
            BindingKind::FutureImport | BindingKind::Export | BindingKind::SubmoduleImport => return None,
            BindingKind::Annotation
            | BindingKind::Argument
            | BindingKind::NamedExpression
            | BindingKind::Assignment
            | BindingKind::TypeParameter
            | BindingKind::LoopVariable
            | BindingKind::WithItem
            | BindingKind::Global(_)
            | BindingKind::Nonlocal(_)
            | BindingKind::Class
            | BindingKind::Function
            | BindingKind::Import
            | BindingKind::FromImport
            | BindingKind::Deletion
            | BindingKind::BoundException
            | BindingKind::UnboundException(_) => target.to_owned(),
        };
        edits.push(TextEdit {
            start: TextPoint::from_point(node.start_position()),
            end: TextPoint::from_point(node.end_position()),
            replacement,
        });
        for reference in &binding.references {
            if text_at(source, reference.at, name.len()) != Some(name) {
                return None;
            }
            edits.push(TextEdit {
                start: TextPoint::from_point(reference.at),
                end: TextPoint::from_point(Point {
                    row: reference.at.row,
                    column: reference.at.column.saturating_add(name.len()),
                }),
                replacement: target.to_owned(),
            });
        }
    }
    edits.sort_by_key(|edit| (edit.start.line, edit.start.column));
    edits.dedup();
    (!edits.is_empty()).then(|| Fix::from_edits(source, edits))
}

fn text_at(source: &[u8], at: Point, length: usize) -> Option<&str> {
    let line_start = source
        .split(|byte| *byte == LINE_FEED)
        .take(at.row)
        .map(|line| line.len().saturating_add(1))
        .sum::<usize>();
    let start = line_start.saturating_add(at.column);
    source
        .get(start..start.saturating_add(length))
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
}

fn is_identifier(name: &str) -> bool {
    let mut letters = name.chars();
    letters
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == PRIVATE_PREFIX)
        && letters.all(|letter| letter.is_alphanumeric() || letter == PRIVATE_PREFIX)
}
