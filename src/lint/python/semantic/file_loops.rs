use tree_sitter::Node;

use super::model::{BindingKind, Model, Resolution};
use super::qualified::{is_typing, qualified_name};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, BODY_FIELD, CALL, COMMENT, EXPRESSION_STATEMENT,
    FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, LEFT_FIELD, OBJECT_FIELD, PARENTHESIZED, RIGHT_FIELD,
    TYPE_FIELD, VALUE_FIELD, named, text, unparenthesized,
};
use crate::lint::python::known_types::is_read_after;
use crate::rules::PythonRule;

const FOR: &str = "for_statement";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const ELSE: &str = "else_clause";
const ASYNC: &str = "async";
const AS_PATTERN: &str = "as_pattern";
const AS_PATTERN_TARGET: &str = "as_pattern_target";
const NAMED_EXPRESSION: &str = "named_expression";
const TYPED_PARAMETERS: [&str; 2] = ["typed_parameter", "typed_default_parameter"];
const WRITE: &str = "write";
const READLINES: &str = "readlines";
const OPEN: &str = "open";
const TYPING_IO: [&str; 3] = ["IO", "BinaryIO", "TextIO"];
const IO_TYPES: [&str; 17] = [
    "io.IOBase",
    "io.RawIOBase",
    "io.BufferedIOBase",
    "io.TextIOBase",
    "io.BytesIO",
    "io.StringIO",
    "io.BufferedReader",
    "io.BufferedWriter",
    "io.BufferedRandom",
    "io.BufferedRWPair",
    "io.TextIOWrapper",
    "os.Path",
    "os.PathLike",
    "pathlib.Path",
    "pathlib.PurePath",
    "pathlib.PurePosixPath",
    "pathlib.PureWindowsPath",
];
const PATH_TYPES: [&str; 4] = [
    "pathlib.Path",
    "pathlib.PurePath",
    "pathlib.PurePosixPath",
    "pathlib.PureWindowsPath",
];
const OPENERS: [&str; 4] = ["io.open", "io.open_code", "os.open", "builtins.open"];

// FURB122: un `for` cuyo cuerpo es sólo `f.write(x)` sobre un archivo, que es `f.writelines(...)`.
// Como ruff, no si el loop pisa una variable de afuera o sus variables se leen después.
pub fn loop_writes(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Option<Report> {
    if statement.child(0).is_some_and(|first| first.kind() == ASYNC)
        || named(statement).iter().any(|child| child.kind() == ELSE)
    {
        return None;
    }
    let body: Vec<Node<'_>> = named(statement.child_by_field_name(BODY_FIELD)?)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    let [only] = body.as_slice() else {
        return None;
    };
    let call = named(*only)
        .first()
        .copied()
        .filter(|call| only.kind() == EXPRESSION_STATEMENT && call.kind() == CALL)?;
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    let writes = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == WRITE);
    let arguments: Vec<Node<'_>> = named(call.child_by_field_name(ARGUMENTS_FIELD)?)
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    let single_positional = matches!(arguments.as_slice(), [argument] if argument.kind() != KEYWORD_ARGUMENT);
    let file = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == IDENTIFIER)?;
    if !writes || !single_positional || !is_io_name(model, source, file) {
        return None;
    }
    let mut names = Vec::new();
    if let Some(target) = statement.child_by_field_name(LEFT_FIELD) {
        target_names(target, &mut names);
    }
    let escapes = names.iter().any(|name| {
        let name = text(*name, source);
        !matches!(model.lookup(name), Resolution::Unresolved) || is_read_after(statement, name, source)
    });
    (!escapes).then(|| Report::at(PythonRule::FURB122, statement, text(file, source)))
}

fn target_names<'tree>(target: Node<'tree>, names: &mut Vec<Node<'tree>>) {
    if target.kind() == IDENTIFIER {
        names.push(target);
        return;
    }
    if target.kind() == ATTRIBUTE || target.kind() == "subscript" {
        return;
    }
    for inner in named(target) {
        target_names(inner, names);
    }
}

// FURB129: `for line in f.readlines():` sobre un archivo, en un `for` o en una comprensión.
pub fn readlines_in_for(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Option<Report> {
    let mut iterable = call;
    while let Some(parent) = iterable.parent().filter(|parent| parent.kind() == PARENTHESIZED) {
        iterable = parent;
    }
    let is_iterable = iterable.parent().is_some_and(|holder| {
        matches!(holder.kind(), FOR | FOR_IN_CLAUSE)
            && holder
                .child_by_field_name(RIGHT_FIELD)
                .is_some_and(|right| right.id() == iterable.id())
    });
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    let reads_lines = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == READLINES);
    let without_arguments = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .is_some_and(|arguments| named(arguments).is_empty());
    if !is_iterable || !reads_lines || !without_arguments {
        return None;
    }
    let object = unparenthesized(function.child_by_field_name(OBJECT_FIELD)?);
    let is_file = if object.kind() == IDENTIFIER {
        is_io_name(model, source, object)
    } else {
        is_io_initializer(model, source, object)
    };
    is_file.then(|| Report::at(PythonRule::FURB129, call, ""))
}

// `typing::is_io_base` de ruff sobre el binding al que resuelve el nombre.
fn is_io_name(model: &Model<'_>, source: &[u8], name: Node<'_>) -> bool {
    let Resolution::Binding(id) = model.lookup(text(name, source)) else {
        return false;
    };
    let Some(binding) = model.binding(id) else {
        return false;
    };
    let Some(identifier) = binding.site.node else {
        return false;
    };
    let Some(parent) = identifier.parent() else {
        return false;
    };
    let is_field = |holder: Node<'_>, field: &str| {
        holder
            .child_by_field_name(field)
            .is_some_and(|value| value.id() == identifier.id())
    };
    let initializer = |field: &str| {
        parent
            .child_by_field_name(field)
            .is_some_and(|value| is_io_initializer(model, source, value))
    };
    let annotated = |holder: Node<'_>| {
        holder
            .child_by_field_name(TYPE_FIELD)
            .is_some_and(|annotation| is_io_annotation(model, source, annotation))
    };
    if binding.kind == BindingKind::Assignment && parent.kind() == ASSIGNMENT && is_field(parent, LEFT_FIELD)
    {
        if parent.child_by_field_name(TYPE_FIELD).is_some() {
            annotated(parent)
        } else {
            initializer(RIGHT_FIELD)
        }
    } else if binding.kind == BindingKind::NamedExpression && parent.kind() == NAMED_EXPRESSION {
        initializer(VALUE_FIELD)
    } else if binding.kind == BindingKind::WithItem && parent.kind() == AS_PATTERN_TARGET {
        parent
            .parent()
            .filter(|pattern| pattern.kind() == AS_PATTERN)
            .and_then(|pattern| named(pattern).first().copied())
            .is_some_and(|value| is_io_initializer(model, source, value))
    } else {
        binding.kind == BindingKind::Argument
            && TYPED_PARAMETERS.contains(&parent.kind())
            && annotated(parent)
    }
}

fn is_io_annotation(model: &Model<'_>, source: &[u8], annotation: Node<'_>) -> bool {
    let annotation = named(annotation)
        .first()
        .copied()
        .filter(|_| annotation.kind() == "type")
        .unwrap_or(annotation);
    qualified_name(model, annotation, source).is_some_and(|name| {
        TYPING_IO.iter().any(|member| is_typing(&name, member)) || IO_TYPES.contains(&name.as_str())
    })
}

// `open(...)`, `io.open(...)` o `Path(...).open(...)`.
fn is_io_initializer(model: &Model<'_>, source: &[u8], value: Node<'_>) -> bool {
    let value = unparenthesized(value);
    if value.kind() != CALL {
        return false;
    }
    let Some(function) = value.child_by_field_name(FUNCTION_FIELD) else {
        return false;
    };
    if function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, source) == OPEN)
        && let Some(object) = function
            .child_by_field_name(OBJECT_FIELD)
            .filter(|object| object.kind() == CALL)
    {
        return object
            .child_by_field_name(FUNCTION_FIELD)
            .and_then(|path| qualified_name(model, path, source))
            .is_some_and(|name| PATH_TYPES.contains(&name.as_str()));
    }
    qualified_name(model, function, source).is_some_and(|name| OPENERS.contains(&name.as_str()))
}
