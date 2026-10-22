use tree_sitter::Node;

use super::collections::{collection_call, config_call};
use super::sinks::{builds_statement, record_sinks, response};
use super::statements::Interpreter;
use super::tables;
use crate::lint::java_receivers::Receiver;
use crate::lint::taint::folding::fold_text_method;
use crate::lint::taint::{Atom, Flow, Literal, State, TaintCall, Value};
use crate::syntax::named_children;

const ABSORBING_METHODS: &[&str] = &[
    "append",
    "extend",
    "add",
    "update",
    "insert",
    "appendleft",
    "write",
];
const ARGUMENTS_FIELD: &str = "arguments";
const ATTRIBUTE_FIELD: &str = "attribute";
const ATTRIBUTE: &str = "attribute";
const BUILTINS: &str = "builtins";
const FUNCTION_FIELD: &str = "function";
const IDENTIFIER: &str = "identifier";
const REQUEST_PATH: &str = "path";
const SPLIT: &str = "split";
const NAME_FIELD: &str = "name";
const KEYWORD_ARGUMENT: &str = "keyword_argument";
const NAME_SEPARATOR: char = '_';
const OBJECT_FIELD: &str = "object";
const QUALIFIER_SEPARATOR: char = '.';
const SELF: &str = "self";
const TEXT_METHODS: &[(&str, &str)] = &[
    ("upper", "toUpperCase"),
    ("lower", "toLowerCase"),
    ("strip", "trim"),
    ("startswith", "startsWith"),
    ("endswith", "endsWith"),
];
const VALUE_FIELD: &str = "value";

pub(super) fn attribute(interpreter: &mut Interpreter<'_, '_>, node: Node<'_>, state: &mut State) -> Value {
    let (Some(object), Some(attribute)) = (
        node.child_by_field_name(OBJECT_FIELD),
        node.child_by_field_name(ATTRIBUTE_FIELD),
    ) else {
        return Value::clean();
    };
    if let Some(known) = interpreter
        .imports
        .qualify(interpreter.text(node))
        .and_then(|qualified| tables::platform_constant(&qualified))
    {
        return Value::Constant(Literal::Text(known.to_owned()));
    }
    if is_request(interpreter, object)
        && interpreter.text(attribute) == REQUEST_PATH
        && let Some(route) = interpreter.route_path.clone()
    {
        return Value::Constant(Literal::Text(route));
    }
    if is_request(interpreter, object) && tables::is_request_attribute(interpreter.text(attribute)) {
        return Value::Data(Flow::of(Atom::Source));
    }
    Value::Data(interpreter.expression(object, state).flow())
}

fn is_request(interpreter: &Interpreter<'_, '_>, object: Node<'_>) -> bool {
    let written = interpreter.text(object);
    tables::is_request_object(written)
        || interpreter
            .imports
            .qualify(written)
            .is_some_and(|qualified| tables::is_request_object(&qualified))
}

// El nombre completo de lo que se llama: por los imports del archivo, las funciones y clases que se
// ven desde acá, `self` dentro de una clase, o la clase con que se creó una variable.
pub(super) fn qualified(interpreter: &Interpreter<'_, '_>, function: Node<'_>) -> Option<String> {
    let written = interpreter.text(function);
    if !written
        .chars()
        .all(|letter| letter.is_alphanumeric() || letter == NAME_SEPARATOR || letter == QUALIFIER_SEPARATOR)
    {
        return None;
    }
    let (first, rest) = written
        .split_once(QUALIFIER_SEPARATOR)
        .map_or((written, None), |(first, rest)| (first, Some(rest)));
    let joined = |prefix: &str| {
        rest.map_or_else(
            || prefix.to_owned(),
            |rest| format!("{prefix}{QUALIFIER_SEPARATOR}{rest}"),
        )
    };
    if first == SELF {
        return interpreter.class.map(joined);
    }
    if let Some(created) = interpreter.instances.get(first) {
        return Some(joined(created));
    }
    if let Some(local) = interpreter.locals.get(first) {
        return Some(joined(local));
    }
    if let Some(imported) = interpreter.imports.qualify(written) {
        return Some(imported);
    }
    (rest.is_none()).then(|| format!("{BUILTINS}{QUALIFIER_SEPARATOR}{written}"))
}

// Los argumentos de una llamada: los posicionales y los que van con nombre.
pub(super) struct Arguments<'source> {
    pub(super) positional: Vec<Value>,
    pub(super) keywords: Vec<(&'source str, Value)>,
}

impl Arguments<'_> {
    pub(super) fn flows(&self) -> Vec<Flow> {
        self.positional
            .iter()
            .chain(self.keywords.iter().map(|(_, value)| value))
            .map(Value::flow)
            .collect()
    }
}

// Lo que se sabe de una llamada antes de decidir qué devuelve.
pub(super) struct Site<'tree, 'source> {
    pub(super) function: Node<'tree>,
    pub(super) object: Option<Node<'tree>>,
    pub(super) method: &'source str,
    pub(super) qualified: Option<String>,
    pub(super) receiver: Option<Value>,
}

fn arguments<'source>(
    interpreter: &mut Interpreter<'source, '_>,
    node: Node<'_>,
    state: &mut State,
) -> Arguments<'source> {
    let mut found = Arguments {
        positional: Vec::new(),
        keywords: Vec::new(),
    };
    for argument in node
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
    {
        if argument.kind() != KEYWORD_ARGUMENT {
            found.positional.push(interpreter.expression(argument, state));
            continue;
        }
        let value = argument
            .child_by_field_name(VALUE_FIELD)
            .map_or_else(Value::clean, |value| interpreter.expression(value, state));
        let name = argument
            .child_by_field_name(NAME_FIELD)
            .map(|name| interpreter.text(name))
            .unwrap_or_default();
        found.keywords.push((name, value));
    }
    found
}

pub(super) fn call(interpreter: &mut Interpreter<'_, '_>, node: Node<'_>, state: &mut State) -> Value {
    let Some(function) = node.child_by_field_name(FUNCTION_FIELD) else {
        return Value::clean();
    };
    let object = (function.kind() == ATTRIBUTE)
        .then(|| function.child_by_field_name(OBJECT_FIELD))
        .flatten();
    let method = match function.child_by_field_name(ATTRIBUTE_FIELD) {
        Some(name) if function.kind() == ATTRIBUTE => interpreter.text(name),
        _ => interpreter.text(function),
    };
    let receiver = object.map(|inner| interpreter.expression(inner, state));
    let arguments = arguments(interpreter, node, state);
    if let Some(name) = object
        .filter(|inner| inner.kind() == IDENTIFIER)
        .map(|inner| interpreter.text(inner))
    {
        let parser = interpreter
            .instances
            .get(name)
            .is_some_and(|created| tables::is_config_parser(created));
        if let Some(value) = state
            .get_mut(name)
            .and_then(|collection| followed_call(collection, parser, method, &arguments.positional))
        {
            return value;
        }
    }
    if let Some(value) = constant_call(receiver.as_ref(), method, &arguments.positional) {
        return value;
    }
    if object.is_some_and(|inner| is_request(interpreter, inner)) && tables::is_request_method(method) {
        return Value::Data(Flow::of(Atom::Source));
    }
    let site = Site {
        function,
        object,
        method,
        qualified: qualified(interpreter, function),
        receiver,
    };
    if let Some(value) = created(site.qualified.as_deref(), &arguments.positional) {
        return value;
    }
    if let Some(body) = response(interpreter, &site, node, &arguments.positional) {
        return Value::Data(body);
    }
    let carried = record_sinks(interpreter, &site, &arguments);
    result(interpreter, &site, carried, state)
}

// Una llamada sobre un `ConfigParser`, una lista o un diccionario que se siguen.
fn followed_call(collection: &mut Value, parser: bool, method: &str, positional: &[Value]) -> Option<Value> {
    match collection {
        Value::Map(entries) if parser => Some(config_call(entries, method, positional)),
        Value::List(_) | Value::Map(_) => Some(collection_call(collection, method, positional)),
        Value::Constant(_) | Value::Data(_) => None,
    }
}

// Un método de `str` sobre un texto constante con argumentos constantes, plegado.
fn constant_call(receiver: Option<&Value>, method: &str, positional: &[Value]) -> Option<Value> {
    let Some(Literal::Text(written)) = receiver.and_then(Value::literal) else {
        return None;
    };
    let constants: Vec<Literal> = positional
        .iter()
        .map(|argument| argument.literal().cloned())
        .collect::<Option<_>>()?;
    if method == SPLIT
        && let [Literal::Text(separator)] = constants.as_slice()
    {
        return Some(Value::List(
            written
                .split(separator.as_str())
                .map(|part| Value::Constant(Literal::Text(part.to_owned())))
                .collect(),
        ));
    }
    let renamed = TEXT_METHODS
        .iter()
        .find(|(python, _)| *python == method)
        .map(|(_, java)| *java)?;
    fold_text_method(written, renamed, &constants).map(Value::Constant)
}

// `dict()`, `list()` y un `ConfigParser()` nuevos, que se siguen por clave o posición.
fn created(qualified: Option<&str>, positional: &[Value]) -> Option<Value> {
    let created = qualified.filter(|_| positional.is_empty())?;
    if tables::creates_map(created) {
        Some(Value::Map(Vec::new()))
    } else if tables::creates_list(created) {
        Some(Value::List(Vec::new()))
    } else {
        None
    }
}

// Lo que devuelve la llamada: limpio, escapado, guardado en el receptor o lo que decida el índice.
fn result(
    interpreter: &mut Interpreter<'_, '_>,
    site: &Site<'_, '_>,
    carried: Vec<Flow>,
    state: &mut State,
) -> Value {
    let qualified = site.qualified.as_deref();
    if qualified.is_some_and(tables::builds_query) || builds_statement(interpreter, site) {
        return Value::clean();
    }
    if let Some(escaped) = qualified.and_then(tables::escapes) {
        return Value::Data(
            carried
                .iter()
                .fold(Flow::default(), Flow::union)
                .without(escaped.bit()),
        );
    }
    if tables::gives_clean_result(site.method) {
        return Value::clean();
    }
    let receiver_flow = site.receiver.as_ref().map(Value::flow).unwrap_or_default();
    if ABSORBING_METHODS.contains(&site.method)
        && let Some(inner) = site.object.filter(|inner| inner.kind() == IDENTIFIER)
    {
        let absorbed = carried.iter().fold(receiver_flow, Flow::union);
        state.insert(interpreter.text(inner).to_owned(), Value::Data(absorbed));
        return Value::clean();
    }
    interpreter.calls.push(TaintCall {
        name: site.method.to_owned(),
        receiver: site.qualified.clone().map_or(Receiver::Unknown, Receiver::Type),
        receiver_flow,
        arguments: carried,
    });
    Value::Data(Flow::of(Atom::Call(interpreter.calls.len().saturating_sub(1))))
}
