use tree_sitter::Node;

use super::model::{Model, ScopeKind};
use super::qualified::qualified_name;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, CLASS, DECORATED, FUNCTION, FUNCTION_FIELD,
    IDENTIFIER, NAME_FIELD, OBJECT_FIELD, VALUE_FIELD, named, text,
};

// `is_open_call` de ruff: lo que abre un archivo o algo parecido que hay que cerrar.
const OPENERS: [&str; 23] = [
    "builtins.open",
    "bz2.open",
    "codecs.open",
    "dbm.open",
    "gzip.open",
    "tarfile.open",
    "shelve.open",
    "tokenize.open",
    "wave.open",
    "dbm.gnu.open",
    "dbm.ndbm.open",
    "dbm.dumb.open",
    "dbm.sqlite3.open",
    "fileinput.FileInput",
    "fileinput.input",
    "io.open",
    "io.open_code",
    "lzma.LZMAFile",
    "lzma.open",
    "tarfile.TarFile.taropen",
    "tempfile.TemporaryFile",
    "tempfile.NamedTemporaryFile",
    "tempfile.SpooledTemporaryFile",
];
// La clase y los métodos que abren algo sobre una instancia recién creada: `pathlib.Path(x).open()`.
const INSTANCE_OPENERS: [(&str, &str); 8] = [
    ("pathlib.Path", "open"),
    ("zipfile.ZipFile", "open"),
    ("lzma.LZMAFile", "open"),
    ("tarfile.TarFile", "open"),
    ("tarfile.TarFile", "taropen"),
    ("tarfile.TarFile", "gzopen"),
    ("tarfile.TarFile", "bz2open"),
    ("tarfile.TarFile", "xzopen"),
];
const TRANSPARENT: [&str; 4] = [
    "parenthesized_expression",
    "argument_list",
    "keyword_argument",
    "pair",
];
const STATEMENT_SUFFIX: &str = "_statement";
const WITH: &str = "with_statement";
const RETURN: &str = "return_statement";
const WITH_CLAUSE: &str = "with_clause";
const WITH_ITEM: &str = "with_item";
const AS_PATTERN: &str = "as_pattern";
const AWAIT: &str = "await";
const CLOSE: &str = "close";
const ENTER_CONTEXT: &str = "enter_context";
const ENTER_ASYNC_CONTEXT: &str = "enter_async_context";
const EXIT_STACK: &str = "contextlib.ExitStack";
const ASYNC_EXIT_STACK: &str = "contextlib.AsyncExitStack";
const UNITTEST_CONTEXTS: [(&str, &str); 3] = [
    ("cls", "enterClassContext"),
    ("self", "enterContext"),
    ("self", "enterAsyncContext"),
];
const ENTER_METHOD: &str = "__enter__";

// SIM115: abrir un archivo fuera de un `with`. Devuelve dónde avisar.
pub fn unmanaged_open<'tree>(model: &Model<'_>, source: &[u8], call: Node<'tree>) -> Option<Node<'tree>> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    if !is_open_call(model, source, function) {
        return None;
    }
    let parent = expression_parent(call);
    let grandparent = parent.and_then(expression_parent);
    let statement = statements(call).next();
    let managed = statement.is_some_and(|statement| matches!(statement.kind(), WITH | RETURN))
        || grandparent.is_some_and(|grandparent| is_close_call(grandparent, source))
        || parent.is_some_and(|parent| {
            is_method_call(parent, ENTER_CONTEXT, source) && inside_with_of(model, source, call, EXIT_STACK)
        })
        || grandparent.is_some_and(|grandparent| {
            grandparent.kind() == AWAIT
                && named(grandparent)
                    .first()
                    .is_some_and(|awaited| is_method_call(*awaited, ENTER_ASYNC_CONTEXT, source))
                && inside_with_of(model, source, call, ASYNC_EXIT_STACK)
        })
        || parent.is_some_and(|parent| is_unittest_context(parent, source))
        || is_inside_enter(model, source, call);
    (!managed).then_some(function)
}

fn is_open_call(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    if let Some(name) = qualified_name(model, function, source) {
        return OPENERS.contains(&name.as_str());
    }
    if function.kind() != ATTRIBUTE {
        return false;
    }
    let method = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .map(|method| text(method, source));
    let class = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == CALL)
        .and_then(|object| object.child_by_field_name(FUNCTION_FIELD))
        .and_then(|class| qualified_name(model, class, source));
    match (class, method) {
        (Some(class), Some(method)) => INSTANCE_OPENERS.contains(&(class.as_str(), method)),
        _ => false,
    }
}

// La expresión que contiene a `node` en el AST de ruff, donde no hay nodos para paréntesis ni
// argumentos.
fn expression_parent(node: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
        .find(|ancestor| !TRANSPARENT.contains(&ancestor.kind()))
}

fn statements(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent).filter(|ancestor| {
        ancestor.kind().ends_with(STATEMENT_SUFFIX) || matches!(ancestor.kind(), FUNCTION | CLASS | DECORATED)
    })
}

fn is_close_call(node: Node<'_>, source: &[u8]) -> bool {
    let no_arguments = node
        .child_by_field_name(ARGUMENTS_FIELD)
        .is_some_and(|arguments| named(arguments).is_empty());
    no_arguments && is_method_call(node, CLOSE, source)
}

fn is_method_call(node: Node<'_>, method: &str, source: &[u8]) -> bool {
    node.kind() == CALL
        && node
            .child_by_field_name(FUNCTION_FIELD)
            .filter(|function| function.kind() == ATTRIBUTE)
            .and_then(|function| function.child_by_field_name(ATTRIBUTE_FIELD))
            .is_some_and(|attribute| text(attribute, source) == method)
}

fn inside_with_of(model: &Model<'_>, source: &[u8], call: Node<'_>, manager: &str) -> bool {
    statements(call)
        .filter(|statement| statement.kind() == WITH)
        .flat_map(named)
        .filter(|child| child.kind() == WITH_CLAUSE)
        .flat_map(named)
        .filter(|item| item.kind() == WITH_ITEM)
        .filter_map(|item| item.child_by_field_name(VALUE_FIELD))
        .map(|value| {
            if value.kind() == AS_PATTERN {
                named(value).first().copied().unwrap_or(value)
            } else {
                value
            }
        })
        .filter(|value| value.kind() == CALL)
        .filter_map(|value| value.child_by_field_name(FUNCTION_FIELD))
        .any(|function| qualified_name(model, function, source).is_some_and(|name| name == manager))
}

fn is_unittest_context(parent: Node<'_>, source: &[u8]) -> bool {
    let Some(function) = Some(parent)
        .filter(|parent| parent.kind() == CALL)
        .and_then(|parent| parent.child_by_field_name(FUNCTION_FIELD))
        .filter(|function| function.kind() == ATTRIBUTE)
    else {
        return false;
    };
    let receiver = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == IDENTIFIER)
        .map(|object| text(object, source));
    let method = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .map(|method| text(method, source));
    matches!((receiver, method), (Some(receiver), Some(method)) if UNITTEST_CONTEXTS.contains(&(receiver, method)))
}

fn is_inside_enter(model: &Model<'_>, source: &[u8], call: Node<'_>) -> bool {
    model.current().kind == ScopeKind::Function
        && std::iter::successors(call.parent(), Node::parent)
            .find(|ancestor| ancestor.kind() == FUNCTION)
            .and_then(|function| function.child_by_field_name(NAME_FIELD))
            .is_some_and(|name| text(name, source) == ENTER_METHOD)
}
