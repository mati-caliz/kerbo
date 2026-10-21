use std::collections::HashMap;

use tree_sitter::Node;

use super::java::{CONSTRUCTOR, METHOD, NAME_FIELD, PRIVATE, STATIC, children, modifiers, text};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const TYPE_DECLARATIONS: &[&str] = &[
    "class_declaration",
    "enum_declaration",
    "record_declaration",
    "interface_declaration",
];
const ENUM: &str = "enum_declaration";
const RECORD: &str = "record_declaration";
const INTERFACE: &str = "interface_declaration";
const FIELD: &str = "field_declaration";
const INTERFACE_FIELD: &str = "constant_declaration";
const ENUM_CONSTANT: &str = "enum_constant";
const ENUM_BODY_DECLARATIONS: &str = "enum_body_declarations";
const FINAL: &str = "final";
const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const EXPLICIT_CONSTRUCTOR: &str = "explicit_constructor_invocation";
const METHOD_REFERENCE: &str = "method_reference";
const ARGUMENTS_FIELD: &str = "arguments";
const PARAMETERS_FIELD: &str = "parameters";
const BODY_FIELD: &str = "body";
const TYPE_FIELD: &str = "type";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const FORMAL_PARAMETER: &str = "formal_parameter";
const SPREAD_PARAMETER: &str = "spread_parameter";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const IDENTIFIER: &str = "identifier";
const FIELD_ACCESS: &str = "field_access";
const UNARY: &str = "unary_expression";
const MINUS: &str = "-";
const THIS: &str = "this";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const LITERALS: &[&str] = &[
    "string_literal",
    "character_literal",
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "true",
    "false",
    "null_literal",
];
const NUMERIC_LITERALS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
];
const QUALIFIER_SEPARATOR: char = '.';

// Lo que se puede llamar de una clase de nivel superior: un método o un constructor, con su nombre,
// la cantidad de parámetros y los nombres de esos parámetros.
struct Callable<'tree> {
    name: String,
    parameters: Vec<Node<'tree>>,
    is_constructor: bool,
    is_candidate: bool,
}

// Una llamada que puede ir a un método o constructor de la clase: el nombre, y por cada argumento
// el valor constante como lo muestra IntelliJ, o nada si no es constante.
struct Call {
    name: String,
    is_constructor: bool,
    arguments: Vec<Option<String>>,
}

// Calca SameParameterValue de IntelliJ para lo privado: un método o constructor privado (también
// el de un enum) al que todas las llamadas de su clase de nivel superior le pasan la misma
// constante en un parámetro. Si hay otro método con el mismo nombre y la misma cantidad de
// parámetros, o alguien lo usa como referencia (`this::metodo`), no se sabe qué llamada va a cuál
// y no se marca. Lo que no es privado pide mirar el proyecto entero.
pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    named_children(root)
        .into_iter()
        .filter(|node| TYPE_DECLARATIONS.contains(&node.kind()))
        .flat_map(|top_level| same_values_in(top_level, source))
        .collect()
}

fn same_values_in(top_level: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let nodes = descendants(top_level);
    let constants = constants_in(&nodes, source);
    let mut callables = Vec::new();
    let mut calls = Vec::new();
    let mut referenced: Vec<&str> = Vec::new();
    for &node in &nodes {
        match node.kind() {
            METHOD | CONSTRUCTOR => callables.extend(callable(node, source)),
            ENUM => callables.extend(implicit_enum_constructor(node, source)),
            RECORD => callables.extend(record_constructor(node, source)),
            INVOCATION | CREATION | EXPLICIT_CONSTRUCTOR | ENUM_CONSTANT => {
                calls.extend(call(node, source, &constants));
            }
            METHOD_REFERENCE => {
                referenced.extend(named_children(node).last().map(|name| text(*name, source)));
            }
            _ => {}
        }
    }
    let mut findings = Vec::new();
    for target in callables.iter().filter(|callable| callable.is_candidate) {
        let arity = target.parameters.len();
        let overloaded = callables
            .iter()
            .filter(|other| {
                other.name == target.name
                    && other.is_constructor == target.is_constructor
                    && other.parameters.len() == arity
            })
            .count()
            > 1;
        if overloaded || arity == 0 || (!target.is_constructor && referenced.contains(&target.name.as_str()))
        {
            continue;
        }
        let matching: Vec<&Call> = calls
            .iter()
            .filter(|call| {
                call.name == target.name
                    && call.is_constructor == target.is_constructor
                    && call.arguments.len() == arity
            })
            .collect();
        if matching.is_empty() {
            continue;
        }
        for (index, parameter) in target.parameters.iter().enumerate() {
            if let Some(value) = shared_value(&matching, index) {
                let name = text(*parameter, source);
                findings.push(finding(
                    LintKind::Java(JavaRule::SameParameterValue),
                    *parameter,
                    &format!("{name} = {value}"),
                    None,
                ));
            }
        }
    }
    findings
}

fn shared_value(calls: &[&Call], index: usize) -> Option<String> {
    let mut values = calls
        .iter()
        .map(|call| call.arguments.get(index).cloned().flatten());
    let first = values.next()??;
    values
        .all(|value| value.as_deref() == Some(first.as_str()))
        .then_some(first)
}

fn callable<'tree>(declaration: Node<'tree>, source: &[u8]) -> Option<Callable<'tree>> {
    let name = text(declaration.child_by_field_name(NAME_FIELD)?, source).to_owned();
    let parameters = declaration.child_by_field_name(PARAMETERS_FIELD)?;
    let written = named_children(parameters);
    let variadic = written
        .iter()
        .any(|parameter| parameter.kind() == SPREAD_PARAMETER);
    let names: Vec<Node<'tree>> = written
        .iter()
        .filter(|parameter| parameter.kind() == FORMAL_PARAMETER)
        .filter_map(|parameter| parameter.child_by_field_name(NAME_FIELD))
        .collect();
    let is_constructor = declaration.kind() == CONSTRUCTOR;
    let private = has_modifier(declaration, PRIVATE, source);
    let has_body = declaration.child_by_field_name(BODY_FIELD).is_some();
    Some(Callable {
        name,
        is_candidate: private && has_body && !variadic,
        parameters: names,
        is_constructor,
    })
}

// Un record trae un constructor con sus componentes: privado si el record lo es.
fn record_constructor<'tree>(declaration: Node<'tree>, source: &[u8]) -> Option<Callable<'tree>> {
    let mut constructor = callable(declaration, source)?;
    constructor.is_constructor = true;
    Some(constructor)
}

// Un enum sin constructor escrito tiene uno implícito sin parámetros: no se marca nada, pero
// cuenta para saber si una constante con argumentos va a otro.
fn implicit_enum_constructor<'tree>(declaration: Node<'tree>, source: &[u8]) -> Option<Callable<'tree>> {
    let body = declaration.child_by_field_name(BODY_FIELD)?;
    let declares_constructor = descendants(body).iter().any(|node| node.kind() == CONSTRUCTOR);
    (!declares_constructor).then(|| Callable {
        name: declaration
            .child_by_field_name(NAME_FIELD)
            .map(|name| text(name, source).to_owned())
            .unwrap_or_default(),
        parameters: Vec::new(),
        is_constructor: true,
        is_candidate: false,
    })
}

// Los constructores de un enum son privados aunque no lo digan.
fn has_modifier(node: Node<'_>, modifier: &str, source: &[u8]) -> bool {
    let written = modifiers(node).is_some_and(|modifiers| {
        children(modifiers)
            .into_iter()
            .any(|child| text(child, source) == modifier)
    });
    written || (modifier == PRIVATE && node.kind() == CONSTRUCTOR && in_enum(node))
}

fn in_enum(constructor: Node<'_>) -> bool {
    constructor
        .parent()
        .and_then(|body| body.parent())
        .and_then(|enum_body| enum_body.parent())
        .is_some_and(|declaration| declaration.kind() == ENUM)
}

fn call(node: Node<'_>, source: &[u8], constants: &Constants) -> Option<Call> {
    let (name, is_constructor) = match node.kind() {
        INVOCATION => (
            text(node.child_by_field_name(NAME_FIELD)?, source).to_owned(),
            false,
        ),
        CREATION => (
            simple_type_name(node.child_by_field_name(TYPE_FIELD)?, source),
            true,
        ),
        _ => (enclosing_type_name(node, source)?, true),
    };
    let arguments = node
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(|arguments| {
            named_children(arguments)
                .into_iter()
                .filter(|argument| !COMMENTS.contains(&argument.kind()))
                .map(|argument| constant_value(argument, source, constants))
                .collect()
        })
        .unwrap_or_default();
    Some(Call {
        name,
        is_constructor,
        arguments,
    })
}

fn simple_type_name(written: Node<'_>, source: &[u8]) -> String {
    let full = text(written, source);
    let without_arguments = full.split('<').next().unwrap_or(full);
    without_arguments
        .rsplit(QUALIFIER_SEPARATOR)
        .next()
        .unwrap_or(without_arguments)
        .trim()
        .to_owned()
}

// `this(...)`, `super(...)` y las constantes de un enum llaman a un constructor de la clase que los
// contiene. Sólo `this(...)` y las constantes cuentan: `super(...)` va a la clase de arriba.
fn enclosing_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == EXPLICIT_CONSTRUCTOR
        && named_children(node)
            .first()
            .is_none_or(|first| first.kind() != THIS)
    {
        return None;
    }
    let mut current = node.parent();
    while let Some(candidate) = current {
        if TYPE_DECLARATIONS.contains(&candidate.kind()) {
            return candidate
                .child_by_field_name(NAME_FIELD)
                .map(|name| text(name, source).to_owned());
        }
        current = candidate.parent();
    }
    None
}

// Por nombre, las clases de este archivo que declaran una constante con ese nombre: un campo
// static final, uno de una interfaz o una constante de un enum.
type Constants = HashMap<String, Vec<String>>;

fn constants_in(nodes: &[Node<'_>], source: &[u8]) -> Constants {
    let mut constants: Constants = HashMap::new();
    for &declaration in nodes
        .iter()
        .filter(|node| TYPE_DECLARATIONS.contains(&node.kind()))
    {
        let Some(owner) = declaration
            .child_by_field_name(NAME_FIELD)
            .map(|name| text(name, source))
        else {
            continue;
        };
        let Some(body) = declaration.child_by_field_name(BODY_FIELD) else {
            continue;
        };
        let interface = declaration.kind() == INTERFACE;
        let members = named_children(body).into_iter().flat_map(|member| {
            if member.kind() == ENUM_BODY_DECLARATIONS {
                named_children(member)
            } else {
                vec![member]
            }
        });
        for member in members {
            let names: Vec<&str> = match member.kind() {
                ENUM_CONSTANT => member
                    .child_by_field_name(NAME_FIELD)
                    .map(|name| text(name, source))
                    .into_iter()
                    .collect(),
                FIELD | INTERFACE_FIELD
                    if interface
                        || member.kind() == INTERFACE_FIELD
                        || (has_modifier(member, STATIC, source) && has_modifier(member, FINAL, source)) =>
                {
                    named_children(member)
                        .into_iter()
                        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                        .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
                        .map(|name| text(name, source))
                        .collect()
                }
                _ => Vec::new(),
            };
            for name in names {
                constants
                    .entry(name.to_owned())
                    .or_default()
                    .push(owner.to_owned());
            }
        }
    }
    constants
}

// El valor como lo escribe IntelliJ: el literal tal cual, y una constante con su clase delante
// (`Clase.NOMBRE`). Lo demás no es constante.
fn constant_value(argument: Node<'_>, source: &[u8], constants: &Constants) -> Option<String> {
    let kind = argument.kind();
    if LITERALS.contains(&kind) {
        return Some(text(argument, source).to_owned());
    }
    match kind {
        UNARY => {
            let operand = argument.child_by_field_name("operand")?;
            let operator = argument.child_by_field_name("operator")?;
            (operator.kind() == MINUS && NUMERIC_LITERALS.contains(&operand.kind()))
                .then(|| format!("{MINUS}{}", text(operand, source)))
        }
        IDENTIFIER => {
            let name = text(argument, source);
            match constants.get(name).map(Vec::as_slice) {
                Some([owner]) => Some(format!("{owner}{QUALIFIER_SEPARATOR}{name}")),
                _ => None,
            }
        }
        FIELD_ACCESS => {
            let owner = argument.child_by_field_name(OBJECT_FIELD)?;
            let name = text(argument.child_by_field_name(FIELD_FIELD)?, source);
            let owner_name = text(owner, source);
            let simple_owner = owner_name
                .rsplit(QUALIFIER_SEPARATOR)
                .next()
                .unwrap_or(owner_name);
            let known = constants
                .get(name)
                .is_some_and(|owners| owners.iter().any(|known| known == simple_owner));
            let looks_constant = simple_owner.starts_with(|first: char| first.is_ascii_uppercase())
                && name
                    .chars()
                    .all(|letter| letter.is_ascii_uppercase() || letter.is_ascii_digit() || letter == '_');
            (known || looks_constant).then(|| format!("{simple_owner}{QUALIFIER_SEPARATOR}{name}"))
        }
        _ => None,
    }
}
