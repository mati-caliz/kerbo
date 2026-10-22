use tree_sitter::Node;

use super::statements::{BODY_FIELD, Interpreter};
use crate::lint::java::NAME_FIELD;
use crate::lint::taint::{State, Value};
use crate::syntax::named_children;

const BLOCK: &str = "block";
const IDENTIFIER: &str = "identifier";
const METHOD: &str = "method_declaration";
const LAMBDA: &str = "lambda_expression";
const PARAMETERS_FIELD: &str = "parameters";

// Lo que una llamada le pasa a su lambda y si lo que la lambda devuelve sale como resultado
// (`map` sí, `orElseThrow` no: su excepción no es el valor).
#[derive(Debug)]
pub(super) struct Callback {
    pub(super) input: Value,
    pub(super) keeps_result: bool,
}

impl Callback {
    pub(super) fn opaque() -> Self {
        Self {
            input: Value::clean(),
            keeps_result: false,
        }
    }
}

// Un argumento: una lambda corre en el lugar; cualquier otro se evalúa.
pub(super) fn argument(
    interpreter: &mut Interpreter<'_, '_>,
    node: Node<'_>,
    state: &mut State,
    callback: &Callback,
) -> Value {
    if node.kind() != LAMBDA {
        return interpreter.expression(node, state);
    }
    let returned = lambda(interpreter, node, state, &callback.input);
    if callback.keeps_result {
        returned
    } else {
        Value::clean()
    }
}

// Una lambda corre en el lugar con lo que captura; sus parámetros valen `input`. Lo que les pasa
// a las variables de afuera (un `add` a una lista capturada) puede haber pasado o no, y lo que
// devuelve es su valor.
pub(super) fn lambda(
    interpreter: &mut Interpreter<'_, '_>,
    node: Node<'_>,
    state: &mut State,
    input: &Value,
) -> Value {
    let Some(body) = node.child_by_field_name(BODY_FIELD) else {
        return Value::clean();
    };
    let parameters: State = node
        .child_by_field_name(PARAMETERS_FIELD)
        .map(|parameters| parameter_names(interpreter, parameters))
        .unwrap_or_default()
        .into_iter()
        .map(|name| (name, input.clone()))
        .collect();
    run_inside(interpreter, body, &parameters, state)
}

// Los métodos de una clase anónima (`new Runnable() { ... }`) ven las variables de afuera; sus
// parámetros, nada conocido.
pub(super) fn anonymous_class(
    interpreter: &mut Interpreter<'_, '_>,
    class_body: Node<'_>,
    state: &mut State,
) {
    for method in named_children(class_body)
        .into_iter()
        .filter(|member| member.kind() == METHOD)
    {
        let (Some(body), Some(parameters)) = (
            method.child_by_field_name(BODY_FIELD),
            method.child_by_field_name(PARAMETERS_FIELD),
        ) else {
            continue;
        };
        let parameters: State = parameter_names(interpreter, parameters)
            .into_iter()
            .map(|name| (name, Value::clean()))
            .collect();
        run_inside(interpreter, body, &parameters, state);
    }
}

// `x`, `(x, y)` o `(String x, int y)`.
fn parameter_names(interpreter: &Interpreter<'_, '_>, parameters: Node<'_>) -> Vec<String> {
    if parameters.kind() == IDENTIFIER {
        return vec![interpreter.text(parameters).to_owned()];
    }
    named_children(parameters)
        .into_iter()
        .filter_map(|parameter| {
            if parameter.kind() == IDENTIFIER {
                Some(parameter)
            } else {
                parameter.child_by_field_name(NAME_FIELD)
            }
        })
        .map(|name| interpreter.text(name).to_owned())
        .collect()
}

fn run_inside(
    interpreter: &mut Interpreter<'_, '_>,
    body: Node<'_>,
    parameters: &State,
    state: &mut State,
) -> Value {
    let mut inner = state.clone();
    inner.extend(
        parameters
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    let outer_returns = std::mem::take(&mut interpreter.returns);
    let outer_exits = std::mem::take(&mut interpreter.exits);
    let after = if body.kind() == BLOCK {
        interpreter.statement(body, inner)
    } else {
        let returned = interpreter.expression(body, &mut inner).flow();
        interpreter.returns.absorb(&returned);
        Some(inner)
    };
    let returned = std::mem::replace(&mut interpreter.returns, outer_returns);
    interpreter.exits = outer_exits;
    for (name, value) in after.into_iter().flatten() {
        if parameters.contains_key(&name) {
            continue;
        }
        if let Some(existing) = state.remove(&name) {
            state.insert(name, existing.join(value));
        }
    }
    Value::Data(returned)
}
