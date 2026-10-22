mod calls;
mod collections;
mod expressions;
mod imports;
mod operators;
mod sinks;
mod statements;
pub mod tables;

use std::collections::HashMap;

use tree_sitter::Node;

use crate::lint::taint::{Atom, Flow, State, TaintMethod, Value};
use crate::syntax::named_children;
use imports::Imports;
use statements::Interpreter;

const FUNCTION: &str = "function_definition";
const CLASS: &str = "class_definition";
const DECORATED: &str = "decorated_definition";
const DECORATOR: &str = "decorator";
const IDENTIFIER: &str = "identifier";
const NAME_FIELD: &str = "name";
const BODY_FIELD: &str = "body";
const PARAMETERS_FIELD: &str = "parameters";
const DEFINITION_FIELD: &str = "definition";
const RECEIVERS: &[&str] = &["self", "cls"];
// Un nombre del archivo: empieza con `@` para que el índice lo complete con el módulo.
const LOCAL_PREFIX: &str = "@";
const QUALIFIER_SEPARATOR: &str = ".";
const DEFINITIONS: &[&str] = &[FUNCTION, CLASS, DECORATED];
// Una vista de Flask devuelve el cuerpo de la respuesta; una de FastAPI, JSON.
const FLASK: &str = "flask";
const FASTAPI: &str = "fastapi";

// Por donde se va: el nombre con que se llega desde el módulo (`clase.metodo`, `init.vista`), la
// clase que encierra y los nombres de funciones y clases que se ven.
#[derive(Clone)]
struct Scope {
    path: String,
    class: Option<String>,
    locals: HashMap<String, String>,
}

// Lo que cada función del archivo hace con los datos, para cruzarlo en el índice.
pub fn python_taints(root: Node<'_>, source: &[u8]) -> Vec<TaintMethod> {
    let imports = Imports::of(root, source);
    let scope = Scope {
        path: String::new(),
        class: None,
        locals: declared_names(root, "", source, &HashMap::new()),
    };
    let mut collector = Collector {
        source,
        imports: &imports,
        methods: Vec::new(),
    };
    collector.visit(root, &scope);
    collector.methods
}

// Lo que necesita el recorrido de un archivo: el código, sus imports y los métodos que va juntando.
struct Collector<'source, 'context> {
    source: &'source [u8],
    imports: &'context Imports,
    methods: Vec<TaintMethod>,
}

fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

fn qualified_name(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}{QUALIFIER_SEPARATOR}{name}")
    }
}

// Las funciones y clases que declara un bloque, con su nombre local para el índice.
fn declared_names(
    block: Node<'_>,
    path: &str,
    source: &[u8],
    outer: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut names = outer.clone();
    for child in named_children(block) {
        let definition = if child.kind() == DECORATED {
            child.child_by_field_name(DEFINITION_FIELD)
        } else {
            Some(child)
        };
        let Some(name) = definition
            .filter(|definition| definition.kind() == FUNCTION || definition.kind() == CLASS)
            .and_then(|definition| definition.child_by_field_name(NAME_FIELD))
        else {
            continue;
        };
        let written = text(name, source);
        names.insert(
            written.to_owned(),
            format!("{LOCAL_PREFIX}{}", qualified_name(path, written)),
        );
    }
    names
}

impl Collector<'_, '_> {
    fn visit(&mut self, block: Node<'_>, scope: &Scope) {
        for child in named_children(block) {
            let (definition, decorators) = if child.kind() == DECORATED {
                let decorators: Vec<String> = named_children(child)
                    .into_iter()
                    .filter(|part| part.kind() == DECORATOR)
                    .map(|part| text(part, self.source).to_owned())
                    .collect();
                (child.child_by_field_name(DEFINITION_FIELD), decorators)
            } else {
                (Some(child), Vec::new())
            };
            let Some(definition) = definition else {
                continue;
            };
            match definition.kind() {
                FUNCTION => self.function(definition, &decorators, scope),
                CLASS => self.class(definition, scope),
                kind if !DEFINITIONS.contains(&kind) => self.visit(definition, scope),
                _ => {}
            }
        }
    }

    fn class(&mut self, definition: Node<'_>, scope: &Scope) {
        let (Some(name), Some(body)) = (
            definition.child_by_field_name(NAME_FIELD),
            definition.child_by_field_name(BODY_FIELD),
        ) else {
            return;
        };
        let path = qualified_name(&scope.path, text(name, self.source));
        let inner = Scope {
            class: Some(format!("{LOCAL_PREFIX}{path}")),
            locals: declared_names(body, &path, self.source, &scope.locals),
            path,
        };
        self.visit(body, &inner);
    }

    fn function(&mut self, definition: Node<'_>, decorators: &[String], scope: &Scope) {
        let (Some(name), Some(body)) = (
            definition.child_by_field_name(NAME_FIELD),
            definition.child_by_field_name(BODY_FIELD),
        ) else {
            return;
        };
        let path = qualified_name(&scope.path, text(name, self.source));
        let mut parameters = parameter_names(definition, self.source);
        if scope.class.is_some()
            && parameters
                .first()
                .is_some_and(|first| RECEIVERS.contains(&first.as_str()))
        {
            parameters.remove(0);
        }
        let route = decorators
            .iter()
            .any(|written| tables::is_route_decorator(written));
        let lambda = tables::is_lambda_handler(&parameters);
        let mut state = State::new();
        for (position, parameter) in parameters.iter().enumerate() {
            let atom = if route || (lambda && position == 0) {
                Atom::Source
            } else {
                Atom::Parameter(position)
            };
            state.insert(parameter.clone(), Value::Data(Flow::of(atom)));
        }
        let locals = declared_names(body, &path, self.source, &scope.locals);
        let mut interpreter = Interpreter::new(self.source, self.imports, &locals, scope.class.as_deref());
        interpreter.route = route && self.imports.uses(FLASK) && !self.imports.uses(FASTAPI);
        interpreter.route_path = decorators.iter().find_map(|written| tables::fixed_route(written));
        interpreter.run(body, state);
        self.methods.push(TaintMethod {
            name: path.clone(),
            owner: scope.class.clone().unwrap_or_default(),
            arity: parameters.len(),
            returns: interpreter.returns,
            calls: interpreter.calls,
            sinks: interpreter.sinks,
        });
        let inner = Scope {
            path,
            class: None,
            locals,
        };
        self.visit(body, &inner);
    }
}

// Los nombres de los parámetros, en orden: `x`, `x: int`, `x=1`, `*args`, `**kwargs`.
fn parameter_names(definition: Node<'_>, source: &[u8]) -> Vec<String> {
    definition
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|parameter| {
            if parameter.kind() == IDENTIFIER {
                return Some(parameter);
            }
            parameter.child_by_field_name(NAME_FIELD).or_else(|| {
                named_children(parameter)
                    .into_iter()
                    .find(|part| part.kind() == IDENTIFIER)
            })
        })
        .map(|name| text(name, source).to_owned())
        .collect()
}
