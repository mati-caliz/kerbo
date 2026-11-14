use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_chain_uses::{field_read_at, nullable_root_arguments};
use super::java_null_checks::{checked_expression, is_stable_call};
use super::java_null_inference::{CallTarget, NullArgument, call_target, cast_type};
use super::java_null_origin::NullReturn;
use super::java_null_returns::returned_by_method;
use super::java_receiver_arguments::receiver_arguments;
use super::java_receivers::{StaticImports, Variables, enclosing_types};
use super::java_throws::ArgumentKind;
use super::java_unboxing::unboxed_at;
use crate::syntax::{named_children, one_based};

const INVOCATION: &str = "method_invocation";
const IDENTIFIER: &str = "identifier";
const FIELD_ACCESS: &str = "field_access";
const QUALIFIER_SEPARATOR: char = '.';
const ASSIGNMENT: &str = "assignment_expression";
const NULL: &str = "null_literal";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const LEFT_FIELD: &str = "left";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const CLASS_LITERAL: &str = "class_literal";
const STRING_LITERAL: &str = "string_literal";
const STANDARD_CHARSETS: &str = "StandardCharsets";
const CHARSET_TYPE: &str = "Charset";
const CHARSET_FACTORIES: &[&str] = &["defaultCharset", "forName"];
const QUOTE: char = '"';

// De dónde sale el primer objeto de una cadena: una variable con su tipo declarado, o una clase
// cuyo método estático se llama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainRoot {
    Instance(String),
    Class(String),
}

// Un paso de una cadena de llamadas: el método, la cantidad de argumentos, dónde está su nombre,
// si su resultado ya se chequeó contra null antes en el archivo y a qué llamadas se pasó antes la
// misma expresión (si alguna exige un parámetro no nulo, IntelliJ la da por no nula después).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainStep {
    pub name: String,
    pub arity: usize,
    pub line: usize,
    pub column: usize,
    pub checked: bool,
    pub class_arguments: Vec<usize>,
    pub text_arguments: Vec<String>,
    pub argument_types: Vec<Option<String>>,
    pub passed_before: Vec<NullArgument>,
}

// `respuesta.getBody().error()`: la raíz y los pasos en orden. Interesan las de al menos dos pasos,
// donde el resultado de una llamada se usa para llamar a otra, y las que terminan desempaquetadas
// (`return jdbc.queryForObject(...)` en un método que devuelve `long`) o leyendo un campo de su
// resultado (`getSubscriber().eventos`), con dónde empiezan, y las
// que se pasan directo como argumento de otra llamada (`Splitter.on(PATH_SEPARATOR.value())`) o
// las devuelve un método no nulo (`return convert(a)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    pub root: ChainRoot,
    pub steps: Vec<ChainStep>,
    pub dereferenced_at: Option<(usize, usize)>,
    pub passed_as: Option<Box<NullArgument>>,
    pub returned: Option<NullReturn>,
    pub nullable_root_arguments: Vec<usize>,
}

// Lo que se sabe del archivo hasta el punto que se recorre: lo que ya se chequeó contra null (por
// texto), las llamadas pasadas como argumento (por texto, con dónde termina la llamada que las
// recibe) y las llamadas que ya son parte de una cadena.
#[derive(Default)]
pub struct NullFacts {
    checked: HashSet<String>,
    passed: HashMap<String, Vec<(usize, NullArgument)>>,
    seen: HashSet<usize>,
}

impl NullFacts {
    // Un chequeo vale dentro del método donde está: al entrar en otro se olvida.
    pub fn enter_method(&mut self) {
        self.checked.clear();
        self.passed.clear();
    }

    // Se llama en orden de recorrido: un chequeo cuenta para lo que viene después.
    pub fn observe(&mut self, node: Node<'_>, source: &[u8], context: (&Variables, &StaticImports)) {
        let (variables, imported) = context;
        self.forget_reassigned(node, source);
        if let Some(checked) = checked_expression(node, source) {
            self.checked.insert(compact(text(checked, source)));
        }
        let arguments = call_arguments(node);
        if !arguments.iter().any(|argument| argument.kind() == INVOCATION) {
            return;
        }
        let Some(CallTarget {
            name,
            is_constructor,
            receiver,
        }) = call_target(node, source, imported, variables)
        else {
            return;
        };
        let receiver_arguments = receiver_arguments(node, source);
        for (index, argument) in arguments.iter().enumerate() {
            if argument.kind() != INVOCATION {
                continue;
            }
            let start = argument.start_position();
            let used = NullArgument {
                name: name.clone(),
                is_constructor,
                arity: arguments.len(),
                receiver: receiver.clone(),
                index,
                line: one_based(start.row),
                column: one_based(start.column),
                cast: None,
                exact: false,
                receiver_arguments: receiver_arguments.clone(),
            };
            self.passed
                .entry(compact(text(*argument, source)))
                .or_default()
                .push((node.end_byte(), used));
        }
    }

    // Un getter ya desreferenciado no vuelve a dar null en el mismo método: IntelliJ sigue la
    // llamada sin argumentos como si fuera un campo, mientras el receptor sea el mismo objeto.
    fn remember_dereferenced(&mut self, invocation: Node<'_>, source: &[u8]) {
        let mut current = invocation;
        while let Some(object) = current.child_by_field_name(OBJECT_FIELD)
            && object.kind() == INVOCATION
        {
            if is_stable_call(object) {
                self.checked.insert(compact(text(object, source)));
            }
            current = object;
        }
    }

    // Asignar una variable borra lo que se sabía de ella y de las llamadas sobre ella.
    fn forget_reassigned(&mut self, node: Node<'_>, source: &[u8]) {
        let Some(variable) = node
            .child_by_field_name(LEFT_FIELD)
            .filter(|left| node.kind() == ASSIGNMENT && left.kind() == IDENTIFIER)
        else {
            return;
        };
        let name = text(variable, source);
        let member_prefix = format!("{name}{QUALIFIER_SEPARATOR}");
        self.checked
            .retain(|known| known != name && !known.starts_with(&member_prefix));
    }

    fn is_checked(&self, expression: Node<'_>, source: &[u8]) -> bool {
        self.checked.contains(&compact(text(expression, source)))
    }

    // La llamada que recibe la expresión como uno de sus argumentos, si la recibe así.
    fn passed_as(&self, expression: Node<'_>, source: &[u8]) -> Option<Box<NullArgument>> {
        let start = expression.start_position();
        self.passed
            .get(&compact(text(expression, source)))?
            .iter()
            .map(|(_, used)| used)
            .find(|used| used.line == one_based(start.row) && used.column == one_based(start.column))
            .map(|used| Box::new(used.clone()))
    }

    fn passed_before(&self, expression: Node<'_>, source: &[u8]) -> Vec<NullArgument> {
        self.passed
            .get(&compact(text(expression, source)))
            .into_iter()
            .flatten()
            .filter(|(call_end, _)| *call_end <= expression.start_byte())
            .map(|(_, used)| used.clone())
            .collect()
    }
}

fn compact(written: &str) -> String {
    written.split_whitespace().collect()
}

// La cadena que termina en esta llamada, si no es parte de otra más larga ya vista. Una variable
// asignada desde una llamada no se sigue: el código real la chequea con `if (x == null) return;` y
// eso pide flujo de datos, no texto.
pub fn chain_of(
    invocation: Node<'_>,
    source: &[u8],
    variables: &Variables,
    facts: &mut NullFacts,
) -> Option<Chain> {
    if facts.seen.contains(&invocation.id()) {
        return None;
    }
    let (root, steps) = walk(invocation, source, variables, facts)?;
    facts.remember_dereferenced(invocation, source);
    let field_read = field_read_at(invocation);
    if field_read.is_some() && is_stable_call(invocation) {
        facts.checked.insert(compact(text(invocation, source)));
    }
    let dereferenced_at = unboxed_at(invocation, source).or(field_read);
    let passed_as = facts.passed_as(invocation, source);
    let returned = returned_by_method(invocation, source);
    let interesting =
        steps.len() > 1 || dereferenced_at.is_some() || passed_as.is_some() || returned.is_some();
    interesting.then_some(Chain {
        root,
        steps,
        dereferenced_at,
        passed_as,
        returned,
        nullable_root_arguments: nullable_root_arguments(invocation, source),
    })
}

// La cadena que da valor a una variable, de un paso o más (`manager.getCache(nombre)`).
pub fn origin_chain(invocation: Node<'_>, source: &[u8], variables: &Variables) -> Option<Chain> {
    let (root, steps) = walk(invocation, source, variables, &mut NullFacts::default())?;
    Some(Chain {
        root,
        steps,
        dereferenced_at: None,
        passed_as: None,
        returned: None,
        nullable_root_arguments: nullable_root_arguments(invocation, source),
    })
}

fn walk(
    invocation: Node<'_>,
    source: &[u8],
    variables: &Variables,
    facts: &mut NullFacts,
) -> Option<(ChainRoot, Vec<ChainStep>)> {
    let mut steps = Vec::new();
    let mut current = invocation;
    loop {
        facts.seen.insert(current.id());
        let name = current.child_by_field_name(NAME_FIELD)?;
        let start = name.start_position();
        steps.push(ChainStep {
            name: text(name, source).to_owned(),
            arity: argument_count(current),
            line: one_based(start.row),
            column: one_based(start.column),
            checked: facts.is_checked(current, source),
            class_arguments: class_arguments(current),
            text_arguments: text_arguments(current, source),
            argument_types: argument_types(current, source, variables),
            passed_before: facts.passed_before(current, source),
        });
        let Some(object) = current.child_by_field_name(OBJECT_FIELD) else {
            steps.reverse();
            let enclosing = enclosing_types(current, source).into_iter().next()?;
            return Some((ChainRoot::Class(enclosing), steps));
        };
        if object.kind() == INVOCATION {
            current = object;
            continue;
        }
        steps.reverse();
        if object.kind() == FIELD_ACCESS {
            let written = text(object, source);
            let qualified_class = written
                .rsplit(QUALIFIER_SEPARATOR)
                .next()
                .is_some_and(|last| last.starts_with(|first: char| first.is_ascii_uppercase()))
                && written.starts_with(|first: char| first.is_ascii_lowercase());
            return qualified_class.then(|| (ChainRoot::Class(written.to_owned()), steps));
        }
        if object.kind() != IDENTIFIER {
            return None;
        }
        let written = text(object, source);
        let root = match variables.get(written) {
            Some(Some(declared)) => ChainRoot::Instance(declared.clone()),
            None if written.starts_with(|first: char| first.is_ascii_uppercase()) => {
                ChainRoot::Class(written.to_owned())
            }
            Some(None) | None => return None,
        };
        return Some((root, steps));
    }
}

// Las posiciones de los argumentos que son un literal de clase (`Long.class`).
fn class_arguments(invocation: Node<'_>) -> Vec<usize> {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .enumerate()
        .filter(|(_, argument)| argument.kind() == CLASS_LITERAL)
        .map(|(position, _)| position)
        .collect()
}

// Lo que dicen los argumentos que son un texto literal (`System.getProperty("java.version")`).
fn text_arguments(invocation: Node<'_>, source: &[u8]) -> Vec<String> {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() == STRING_LITERAL)
        .map(|literal| text(literal, source).trim_matches(QUOTE).to_owned())
        .collect()
}

// El tipo de cada argumento cuando se sabe: una variable con su tipo declarado o un casteo.
fn argument_types(invocation: Node<'_>, source: &[u8], variables: &Variables) -> Vec<Option<String>> {
    call_arguments(invocation)
        .into_iter()
        .map(|argument| {
            if argument.kind() == IDENTIFIER {
                return variables.get(text(argument, source)).cloned().flatten();
            }
            cast_type(argument, source)
        })
        .collect()
}

pub(super) fn call_arguments(invocation: Node<'_>) -> Vec<Node<'_>> {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .collect()
}

pub(super) fn argument_kind(argument: Node<'_>, source: &[u8]) -> ArgumentKind {
    match argument.kind() {
        STRING_LITERAL => ArgumentKind::Text,
        NULL => ArgumentKind::Null,
        FIELD_ACCESS if names_standard_charsets(argument.child_by_field_name(OBJECT_FIELD), source) => {
            ArgumentKind::Charset
        }
        INVOCATION if is_charset_factory(argument, source) => ArgumentKind::Charset,
        _ => ArgumentKind::Unknown,
    }
}

fn names_standard_charsets(object: Option<Node<'_>>, source: &[u8]) -> bool {
    object.is_some_and(|node| {
        text(node, source)
            .rsplit(QUALIFIER_SEPARATOR)
            .next()
            .is_some_and(|last| last == STANDARD_CHARSETS)
    })
}

fn is_charset_factory(invocation: Node<'_>, source: &[u8]) -> bool {
    let named = invocation
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| CHARSET_FACTORIES.contains(&text(name, source)));
    named
        && invocation
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| text(object, source) == CHARSET_TYPE)
}

pub(super) fn argument_count(invocation: Node<'_>) -> usize {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .count()
}
