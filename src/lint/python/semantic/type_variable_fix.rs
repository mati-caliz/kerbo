use tree_sitter::{Node, Point};

use super::definitions::type_parameter_parts;
use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::model::{BindingId, BindingKind, Model};
use super::parameters::annotation;
use super::type_hints::unwrapped;
use crate::fix::{Fix, TextEdit};
use crate::lint::python::Report;
use crate::lint::python::ast::{COMMENT, DECORATED, NAME_FIELD, named, text};
use crate::lint::python::fix_edits::point_of;
use crate::rules::PythonRule;

const TYPING: &str = "typing";
const SELF: &str = "Self";
const TYPE_PARAMETERS_FIELD: &str = "type_parameters";
const LINE_FEED: u8 = b'\n';

// La variable de tipo de un PYI019: un `X = TypeVar(...)` o un parámetro de tipo del método.
#[derive(Debug, Clone, Copy)]
pub enum TypeVariable<'tree> {
    Assigned(BindingId),
    Parameter(Node<'tree>),
}

#[derive(Debug, Clone, Copy)]
pub struct SelfTypeVariable<'tree> {
    pub function: Node<'tree>,
    pub first: Node<'tree>,
    pub variable: TypeVariable<'tree>,
}

// Los PYI019 que se avisaron recorriendo el árbol reciben su arreglo al final, con todas las
// referencias a la variable ya registradas (ruff lo calcula al analizar los bindings).
pub fn attach_self_type_variable_fixes(
    model: &Model<'_>,
    source: &[u8],
    reports: &mut [Report],
    variables: &[SelfTypeVariable<'_>],
) {
    for variable in variables {
        let Some(name) = variable.function.child_by_field_name(NAME_FIELD) else {
            continue;
        };
        let start = name.end_position();
        if let Some(report) = reports
            .iter_mut()
            .find(|report| report.rule == PythonRule::PYI019 && report.start == start && report.fix.is_none())
        {
            report.fix = self_type_variable_fix(model, source, *variable);
        }
    }
}

// El `replace_custom_typevar_with_self` de ruff: importa `Self`, le saca la anotación al primer
// parámetro, saca el parámetro de tipo si lo era y cambia por `Self` cada uso de la variable después
// de esa anotación y dentro del método. Si algún uso no está escrito tal cual (un string con escapes)
// no hay arreglo; es inseguro si alguna edición se lleva un comentario.
pub fn self_type_variable_fix(
    model: &Model<'_>,
    source: &[u8],
    variable: SelfTypeVariable<'_>,
) -> Option<Fix> {
    let function = variable.function;
    let binding = model.binding(binding_of(model, variable.variable)?)?;
    let variable_name = text(binding.site.node?, source);
    let holder = function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .unwrap_or(function);
    let request = ImportRequest {
        module: TYPING,
        member: SELF,
        style: ImportStyle::ImportFrom,
    };
    let (self_name, import) = get_or_import_member(model, source, holder, request)?;
    let annotation = unwrapped(annotation(variable.first)?);
    let parameter_name = variable
        .first
        .child_by_field_name(NAME_FIELD)
        .or_else(|| named(variable.first).into_iter().next())?;
    let mut ranges = vec![(parameter_name.end_byte(), annotation.end_byte(), String::new())];
    if let TypeVariable::Parameter(parameter) = variable.variable {
        let (start, end) = parameter_removal(function, parameter)?;
        ranges.push((start, end, String::new()));
    }
    for reference in &binding.references {
        let start = byte_at(source, reference.at);
        let end = start.saturating_add(variable_name.len());
        if source.get(start..end) != Some(variable_name.as_bytes()) {
            return None;
        }
        if start >= annotation.end_byte() && end <= function.end_byte() {
            ranges.push((start, end, self_name.clone()));
        }
    }
    let comments = comments_in(holder);
    let takes_comment = ranges.iter().any(|(start, end, _)| {
        comments
            .iter()
            .any(|comment| comment.start_byte() >= *start && comment.end_byte() <= *end)
    });
    let mut edits = vec![import];
    edits.extend(ranges.into_iter().map(|(start, end, replacement)| TextEdit {
        start: point_of(source, start),
        end: point_of(source, end),
        replacement,
    }));
    let fix = Fix::from_edits(source, edits);
    Some(if takes_comment { fix.marked_unsafe() } else { fix })
}

fn binding_of(model: &Model<'_>, variable: TypeVariable<'_>) -> Option<BindingId> {
    match variable {
        TypeVariable::Assigned(id) => Some(id),
        TypeVariable::Parameter(parameter) => {
            let name = type_parameter_parts(parameter).0?;
            model.bindings.iter().position(|binding| {
                binding.kind == BindingKind::TypeParameter
                    && binding.site.node.is_some_and(|node| node.id() == name.id())
            })
        }
    }
}

// `remove_pep695_typevar_declaration`: la lista entera si es el único; si no, hasta el siguiente o
// desde el final del anterior si es el último.
fn parameter_removal(function: Node<'_>, parameter: Node<'_>) -> Option<(usize, usize)> {
    let list = function.child_by_field_name(TYPE_PARAMETERS_FIELD)?;
    let parameters: Vec<Node<'_>> = named(list)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    if let [only] = parameters.as_slice() {
        return (only.id() == parameter.id()).then(|| (list.start_byte(), list.end_byte()));
    }
    let index = parameters.iter().position(|other| other.id() == parameter.id())?;
    if let Some(next) = parameters.get(index.saturating_add(1)) {
        return Some((parameter.start_byte(), next.start_byte()));
    }
    let previous = parameters.get(index.checked_sub(1)?)?;
    Some((previous.end_byte(), parameter.end_byte()))
}

fn comments_in(node: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current.kind() == COMMENT {
            found.push(current);
        }
        let mut cursor = current.walk();
        pending.extend(current.children(&mut cursor));
    }
    found
}

fn byte_at(source: &[u8], at: Point) -> usize {
    let line_start = source
        .split(|byte| *byte == LINE_FEED)
        .take(at.row)
        .map(|line| line.len().saturating_add(1))
        .sum::<usize>();
    line_start.saturating_add(at.column)
}
