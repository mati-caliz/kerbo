use tree_sitter::{Node, Point};

use super::importer::{ImportRequest, get_or_import_member};
use super::model::{Model, Resolution};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, CALL, FALSE, IDENTIFIER, SUBSCRIPT, TRUE, is_bytes, is_string, text,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::fix_edits::{Parentheses, commented_unsafe, pad, remove_argument};

const OPEN: &str = "open";
const BUILTINS: &str = "builtins";
const STR: &str = "str";
const PLAIN_ATTRIBUTE_OBJECTS: [&str; 4] = [IDENTIFIER, ATTRIBUTE, SUBSCRIPT, CALL];
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];

// `has_builtin_binding` de ruff: el nombre no está tapado por nada del archivo.
pub fn builtin_available(model: &Model<'_>, name: &str) -> bool {
    model.lookup_from(model.scope, name) == Resolution::Builtin
}

// PIE808: `range(0, n)` pasa a `range(n)`.
pub fn range_start_fix(call: Node<'_>, start: Node<'_>, source: &[u8]) -> Option<Fix> {
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    Some(remove_argument(start, arguments, Parentheses::Preserve, source))
}

// B009: `getattr(x, "a")` pasa a `x.a`; lo que no es un nombre, un atributo, un subíndice o una
// llamada de una línea va entre paréntesis. Inseguro con comentarios o un nombre no ASCII (ruff lo
// mira con NFKC).
pub fn getattr_fix(call: Node<'_>, object: Node<'_>, name: Node<'_>, source: &[u8]) -> Option<Fix> {
    let attribute = text_value(name, source)?;
    let object_text = text(object, source);
    let content = if PLAIN_ATTRIBUTE_OBJECTS.contains(&object.kind()) && !object_text.contains('\n') {
        format!("{object_text}.{attribute}")
    } else {
        format!("({object_text}).{attribute}")
    };
    let fix = Fix::replace(call, &pad(content, call, source), source);
    let fix = commented_unsafe(fix, call, source);
    Some(if attribute.is_ascii() {
        fix
    } else {
        fix.marked_unsafe()
    })
}

// UP008: `super(Clase, self)` pasa a `super()`.
pub fn super_arguments_fix(arguments: Node<'_>, source: &[u8]) -> Fix {
    let start = arguments.start_position();
    let end = arguments.end_position();
    let inside_start = Point {
        row: start.row,
        column: start.column.saturating_add(1),
    };
    let inside_end = Point {
        row: end.row,
        column: end.column.saturating_sub(1),
    };
    commented_unsafe(
        Fix::replace_range(inside_start, inside_end, "", source),
        arguments,
        source,
    )
}

// UP020: `io.open` pasa a `open`, o a `builtins.open` si el builtin está tapado.
pub fn open_alias_fix(model: &Model<'_>, function: Node<'_>, source: &[u8]) -> Option<Fix> {
    let fix = builtin_fix(model, function, OPEN, source)?;
    Some(commented_unsafe(fix, function, source))
}

// `get_or_import_builtin_symbol` de ruff: el builtin si está libre; si no, `builtins.nombre` con el
// import que haga falta.
pub fn builtin_fix(model: &Model<'_>, node: Node<'_>, builtin: &str, source: &[u8]) -> Option<Fix> {
    if builtin_available(model, builtin) {
        return Some(Fix::replace(node, builtin, source));
    }
    let request = ImportRequest {
        module: BUILTINS,
        member: builtin,
    };
    let (name, import) = get_or_import_member(model, source, node, request)?;
    let reference = TextEdit {
        start: TextPoint::from_point(node.start_position()),
        end: TextPoint::from_point(node.end_position()),
        replacement: name,
    };
    Some(Fix::from_edits(source, vec![reference, import]))
}

// UP019: `typing.Text` pasa a `str`.
pub fn text_alias_fix(model: &Model<'_>, node: Node<'_>, source: &[u8]) -> Option<Fix> {
    builtin_fix(model, node, STR, source)
}

// UP003: `type("")` pasa a `str`, con un espacio si queda pegado a una letra.
pub fn type_of_primitive_fix(
    model: &Model<'_>,
    call: Node<'_>,
    argument: Node<'_>,
    source: &[u8],
) -> Option<Fix> {
    let is_imaginary = text(argument, source).ends_with(IMAGINARY_SUFFIXES);
    let builtin = match argument.kind() {
        TRUE | FALSE => "bool",
        INTEGER | FLOAT if is_imaginary => "complex",
        INTEGER => "int",
        FLOAT => "float",
        _ if is_bytes(argument, source) => "bytes",
        _ if is_string(argument) => STR,
        _ => return None,
    };
    builtin_available(model, builtin)
        .then(|| Fix::replace(call, &pad(builtin.to_owned(), call, source), source))
}
