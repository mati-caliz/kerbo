mod calls;
mod expressions;
mod imports;
mod statements;
pub mod tables;

use std::collections::HashMap;

use tree_sitter::Node;

use crate::lint::taint::{Atom, Flow, State, TaintMethod, Value};
use crate::syntax::{descendants, named_children};
use imports::Imports;
use statements::Interpreter;

const FUNCTIONS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "function_expression",
    "function",
    "arrow_function",
    "method_definition",
];
const DECLARATOR: &str = "variable_declarator";
const CLASS_BODY: &str = "class_body";
const CLASS_DECLARATION: &str = "class_declaration";
const NAME_FIELD: &str = "name";
const BODY_FIELD: &str = "body";
const PARAMETERS_FIELD: &str = "parameters";
const PARAMETER_FIELD: &str = "parameter";
const PATTERN_FIELD: &str = "pattern";
const ASSIGNMENT: &str = "assignment_expression";
const LEFT_FIELD: &str = "left";
// Un nombre del archivo: empieza con `@` para que el índice lo complete con el módulo.
const LOCAL_PREFIX: &str = "@";
const QUALIFIER_SEPARATOR: &str = ".";
const ANONYMOUS_PREFIX: &str = "<anónima>:";

// Lo que cada función del archivo hace con los datos, para cruzarlo en el índice. Las anónimas (el
// `(req, res) => { ... }` de una ruta de Express) también, con su línea como nombre.
pub fn ecmascript_taints(root: Node<'_>, source: &[u8]) -> Vec<TaintMethod> {
    let imports = Imports::of(root, source);
    let nodes = descendants(root);
    let locals: HashMap<String, String> = nodes
        .iter()
        .filter(|node| FUNCTIONS.contains(&node.kind()))
        .filter_map(|function| function_name(*function, source))
        .map(|name| (name.clone(), format!("{LOCAL_PREFIX}{name}")))
        .collect();
    nodes
        .iter()
        .filter(|node| FUNCTIONS.contains(&node.kind()))
        .filter_map(|function| summary(*function, source, &imports, &locals))
        .collect()
}

fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

// El nombre con que se llama a una función desde el archivo: el suyo, el de la constante que la
// guarda, o `Clase.metodo`.
fn function_name(function: Node<'_>, source: &[u8]) -> Option<String> {
    if let Some(name) = function.child_by_field_name(NAME_FIELD) {
        let class = function
            .parent()
            .filter(|parent| parent.kind() == CLASS_BODY)
            .and_then(|body| body.parent())
            .filter(|class| class.kind() == CLASS_DECLARATION)
            .and_then(|class| class.child_by_field_name(NAME_FIELD));
        let own = text(name, source);
        return Some(class.map_or_else(
            || own.to_owned(),
            |class| format!("{}{QUALIFIER_SEPARATOR}{own}", text(class, source)),
        ));
    }
    function
        .parent()
        .filter(|parent| parent.kind() == DECLARATOR)
        .and_then(|declarator| declarator.child_by_field_name(NAME_FIELD))
        .map(|name| text(name, source).to_owned())
}

fn summary(
    function: Node<'_>,
    source: &[u8],
    imports: &Imports,
    locals: &HashMap<String, String>,
) -> Option<TaintMethod> {
    let body = function.child_by_field_name(BODY_FIELD)?;
    let parameters = parameter_patterns(function);
    let mut interpreter = Interpreter::new(source, imports, locals);
    let mut state = State::new();
    let written: Vec<&str> = parameters
        .iter()
        .map(|parameter| text(*parameter, source))
        .collect();
    let declared_name = function_name(function, source).or_else(|| assigned_name(function, source));
    let lambda = tables::is_lambda_handler(declared_name.as_deref(), &written);
    interpreter.lambda = lambda;
    for (position, parameter) in parameters.iter().enumerate() {
        let atom = if lambda && position == 0 {
            Atom::Source
        } else {
            Atom::Parameter(position)
        };
        interpreter.bind(*parameter, &Value::Data(Flow::of(atom)), &mut state);
    }
    interpreter.run(body, state);
    let start = function.start_position();
    let name = function_name(function, source).unwrap_or_else(|| format!("{ANONYMOUS_PREFIX}{}", start.row));
    Some(TaintMethod {
        name,
        owner: String::new(),
        arity: parameters.len(),
        returns: interpreter.returns,
        calls: interpreter.calls,
        sinks: interpreter.sinks,
    })
}

// `exports.handler = async (event) => { ... }`: el nombre a la izquierda de la asignación.
fn assigned_name(function: Node<'_>, source: &[u8]) -> Option<String> {
    function
        .parent()
        .filter(|parent| parent.kind() == ASSIGNMENT)
        .and_then(|assignment| assignment.child_by_field_name(LEFT_FIELD))
        .map(|left| text(left, source).to_owned())
}

// Los parámetros, en orden: `x`, `x: T`, `x = 1`, `{ a, b }` o el único de `x => ...`.
fn parameter_patterns(function: Node<'_>) -> Vec<Node<'_>> {
    if let Some(single) = function.child_by_field_name(PARAMETER_FIELD) {
        return vec![single];
    }
    function
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .map(|parameter| parameter.child_by_field_name(PATTERN_FIELD).unwrap_or(parameter))
        .collect()
}
