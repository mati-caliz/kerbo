use tree_sitter::Node;

use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::model::Model;
use crate::fix::{Fix, TextEdit};
use crate::lint::python::ast::{
    ASSIGNMENT, BODY_FIELD, CLASS, DECORATED, EXPRESSION_STATEMENT, FUNCTION, IDENTIFIER, LEFT_FIELD,
    NAME_FIELD, TYPE_FIELD, VALUE_FIELD, named, text,
};
use crate::lint::python::fix_edits::point_of;

const DATACLASSES: &str = "dataclasses";
const INIT_VAR: &str = "InitVar";
const TYPE_PARAMETERS_FIELD: &str = "type_parameters";
const DEFINITION_FIELD: &str = "definition";
const LINE_FEED: u8 = b'\n';
const HORIZONTAL_SPACE: &[u8] = b" \t";

pub struct PostInit<'tree> {
    pub class: Node<'tree>,
    pub function: Node<'tree>,
}

// El `use_initvar` de ruff: el default se borra del parámetro y el parámetro pasa a ser un campo
// `InitVar` de la clase, justo arriba de `__post_init__`. Sin arreglo si la clase ya tiene un nombre
// así o si la anotación usa un parámetro de tipo del propio `__post_init__`.
pub fn initvar_fix(
    model: &Model<'_>,
    source: &[u8],
    post_init: &PostInit<'_>,
    parameter: Node<'_>,
) -> Option<Fix> {
    let name = parameter.child_by_field_name(NAME_FIELD)?;
    let default = parameter.child_by_field_name(VALUE_FIELD)?;
    let annotation = parameter.child_by_field_name(TYPE_FIELD);
    if class_binds(post_init.class, text(name, source), source)
        || annotation
            .is_some_and(|annotation| uses_own_type_parameter(post_init.function, annotation, source))
    {
        return None;
    }
    let request = ImportRequest {
        module: DATACLASSES,
        member: INIT_VAR,
        style: ImportStyle::Import,
    };
    let (init_var, import) = get_or_import_member(model, source, default, request)?;
    let statement = post_init
        .function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .unwrap_or(post_init.function);
    let line_start = line_start(source, statement.start_byte());
    let indentation = source.get(line_start..statement.start_byte())?;
    if !indentation.iter().all(|byte| HORIZONTAL_SPACE.contains(byte)) {
        return None;
    }
    let indentation = String::from_utf8_lossy(indentation);
    let parameter_name = text(name, source);
    let default_text = text(default, source);
    let field = annotation.map_or_else(
        || format!("{indentation}{parameter_name}: {init_var} = {default_text}\n"),
        |annotation| {
            let annotation = text(annotation, source);
            format!("{indentation}{parameter_name}: {init_var}[{annotation}] = {default_text}\n")
        },
    );
    let kept_end = annotation.unwrap_or(name).end_byte();
    let default_removal = TextEdit {
        start: point_of(source, kept_end),
        end: point_of(source, default.end_byte()),
        replacement: String::new(),
    };
    let field_insertion = TextEdit {
        start: point_of(source, line_start),
        end: point_of(source, line_start),
        replacement: field,
    };
    Some(Fix::from_edits(source, vec![import, default_removal, field_insertion]).marked_unsafe())
}

fn class_binds(class: Node<'_>, name: &str, source: &[u8]) -> bool {
    class
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .any(|statement| bound_names(statement, source).contains(&name))
}

fn bound_names<'source>(statement: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    let definition = if statement.kind() == DECORATED {
        statement
            .child_by_field_name(DEFINITION_FIELD)
            .unwrap_or(statement)
    } else {
        statement
    };
    match definition.kind() {
        FUNCTION | CLASS => definition
            .child_by_field_name(NAME_FIELD)
            .map(|name| vec![text(name, source)])
            .unwrap_or_default(),
        EXPRESSION_STATEMENT => named(definition)
            .into_iter()
            .filter(|expression| expression.kind() == ASSIGNMENT)
            .filter_map(|assignment| assignment.child_by_field_name(LEFT_FIELD))
            .filter(|target| target.kind() == IDENTIFIER)
            .map(|target| text(target, source))
            .collect(),
        _ => Vec::new(),
    }
}

fn uses_own_type_parameter(function: Node<'_>, annotation: Node<'_>, source: &[u8]) -> bool {
    let Some(type_parameters) = function.child_by_field_name(TYPE_PARAMETERS_FIELD) else {
        return false;
    };
    let declared: Vec<&str> = identifiers(type_parameters)
        .into_iter()
        .map(|identifier| text(identifier, source))
        .collect();
    identifiers(annotation)
        .into_iter()
        .any(|identifier| declared.contains(&text(identifier, source)))
}

fn identifiers(node: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current.kind() == IDENTIFIER {
            found.push(current);
        }
        pending.extend(named(current));
    }
    found
}

fn line_start(source: &[u8], offset: usize) -> usize {
    source
        .get(..offset)
        .and_then(|before| before.iter().rposition(|byte| *byte == LINE_FEED))
        .map_or(0, |position| position.saturating_add(1))
}
