mod constants;
mod pending;

use std::collections::HashMap;

use tree_sitter::Node;

use crate::syntax::{children_with_fields, descendants, named_children, one_based};
use constants::constant_specs;
use pending::{FileNames, has_constants, message_pieces, package_constants};
pub use pending::{MessagePiece, PendingMessage};

const CALL: &str = "call_expression";
const SELECTOR: &str = "selector_expression";
const IDENTIFIER: &str = "identifier";
const IMPORT_SPEC: &str = "import_spec";
const PACKAGE_CLAUSE: &str = "package_clause";
const FUNCTION: &str = "function_declaration";
const METHOD: &str = "method_declaration";
const TYPE_DECLARATION: &str = "type_declaration";
const FUNCTIONS: &[&str] = &["function_declaration", "method_declaration", "func_literal"];
const VAR_DECLARATION: &str = "var_declaration";
const DECLARED_SPECS: &[&str] = &["type_spec", "type_alias", "var_spec"];
const POINTER_TYPE: &str = "pointer_type";
const PARENTHESIZED_TYPE: &str = "parenthesized_type";
const GENERIC_TYPE: &str = "generic_type";
const RECEIVER_FIELD: &str = "receiver";
const TYPE_FIELD: &str = "type";
const FUNCTION_FIELD: &str = "function";
const ARGUMENTS_FIELD: &str = "arguments";
const OPERAND_FIELD: &str = "operand";
const FIELD_FIELD: &str = "field";
const NAME_FIELD: &str = "name";
const PATH_FIELD: &str = "path";
const COMMENT: &str = "comment";
const PATH_SEPARATOR: char = '/';
const QUOTE: char = '"';
const WORD_SEPARATOR: char = ' ';
const CALL_MARK: char = '(';
// Lo que staticcheck mira: `errors.New` y `fmt.Errorf`, por la ruta del import.
const ERROR_CONSTRUCTORS: &[(&str, &str)] = &[("errors", "New"), ("fmt", "Errorf")];
const FINAL_PUNCTUATION: &[char] = &['.', ':', '!', '\n'];

// Un mensaje de error que empieza con mayúscula, con su primera palabra: si es el nombre de algo del
// paquete (una función, un tipo, un método), staticcheck no avisa, y eso sólo se sabe con el paquete
// entero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalizedError {
    pub line: usize,
    pub column: usize,
    pub word: String,
}

// Lo que un archivo aporta a ST1005 a nivel de paquete. Los mensajes fuera de toda función (en la
// inicialización de una variable del paquete) se miran aunque el archivo sea un test.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoPackageFacts {
    pub package: String,
    pub declared: Vec<String>,
    pub capitalized: Vec<CapitalizedError>,
    pub outside_functions: Vec<(usize, usize)>,
    pub constants: Vec<(String, String)>,
    pub pending: Vec<PendingMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorStringProblem {
    Punctuation,
    Capitalized,
    Unresolved,
}

#[derive(Debug)]
pub struct ErrorString<'tree> {
    pub call: Node<'tree>,
    pub callee: String,
    pub problem: ErrorStringProblem,
}

// `CheckErrorStrings` de staticcheck (ST1005), sobre los mensajes constantes del archivo. Los que
// nombran constantes de otro archivo quedan pendientes para la pasada por paquete.
pub fn error_strings<'tree>(root: Node<'tree>, source: &[u8]) -> (Vec<ErrorString<'tree>>, GoPackageFacts) {
    let imports = imported_packages(root, source);
    let names = FileNames {
        constants: constant_specs(root, source),
    };
    let mut collected = Collected::default();
    for call in descendants(root).into_iter().filter(|node| node.kind() == CALL) {
        let Some(callee) = constructor(call, &imports, source) else {
            continue;
        };
        let Some(pieces) = call
            .child_by_field_name(ARGUMENTS_FIELD)
            .and_then(|arguments| {
                named_children(arguments)
                    .into_iter()
                    .find(|argument| argument.kind() != COMMENT)
            })
            .and_then(|first| message_pieces(first, source, &names))
        else {
            continue;
        };
        collected.add(call, callee, pieces);
    }
    let facts = GoPackageFacts {
        package: package_name(root, source),
        declared: declared_names(root, source),
        capitalized: collected.capitalized,
        outside_functions: collected.outside_functions,
        constants: package_constants(root, source, &names),
        pending: collected.pending,
    };
    (collected.found, facts)
}

#[derive(Default)]
struct Collected<'tree> {
    found: Vec<ErrorString<'tree>>,
    capitalized: Vec<CapitalizedError>,
    outside_functions: Vec<(usize, usize)>,
    pending: Vec<PendingMessage>,
}

impl<'tree> Collected<'tree> {
    fn add(&mut self, call: Node<'tree>, callee: String, pieces: Vec<MessagePiece>) {
        let start = call.start_position();
        let (line, column) = (one_based(start.row), one_based(start.column));
        if !inside_a_function(call) {
            self.outside_functions.push((line, column));
        }
        let mut report = |problem: ErrorStringProblem| {
            self.found.push(ErrorString {
                call,
                callee: callee.clone(),
                problem,
            });
        };
        if has_constants(&pieces) {
            report(ErrorStringProblem::Punctuation);
            report(ErrorStringProblem::Unresolved);
            self.pending.push(PendingMessage {
                line,
                column,
                callee,
                pieces,
            });
            return;
        }
        let message: String = pieces
            .iter()
            .filter_map(|piece| match piece {
                MessagePiece::Text(text) => Some(text.as_str()),
                MessagePiece::Constant(_) => None,
            })
            .collect();
        let (punctuated, word) = message_problems(&message);
        if punctuated {
            report(ErrorStringProblem::Punctuation);
        }
        if let Some(word) = word {
            report(ErrorStringProblem::Capitalized);
            self.capitalized.push(CapitalizedError { line, column, word });
        }
    }
}

// Lo que ST1005 mira de un mensaje: si termina en puntuación y la primera palabra con mayúscula.
pub fn message_problems(message: &str) -> (bool, Option<String>) {
    (message.ends_with(FINAL_PUNCTUATION), capitalized_word(message))
}

fn package_name(root: Node<'_>, source: &[u8]) -> String {
    named_children(root)
        .into_iter()
        .find(|node| node.kind() == PACKAGE_CLAUSE)
        .and_then(|clause| named_children(clause).into_iter().next())
        .and_then(|name| name.utf8_text(source).ok())
        .unwrap_or_default()
        .to_owned()
}

fn inside_a_function(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if FUNCTIONS.contains(&parent.kind()) {
            return true;
        }
        current = parent.parent();
    }
    false
}

// La primera palabra si empieza con mayúscula y no parece una sigla, un nombre compuesto ni una
// llamada; sin espacio, es un mensaje de una palabra y no cuenta.
fn capitalized_word(message: &str) -> Option<String> {
    let (word, _) = message.split_once(WORD_SEPARATOR)?;
    let mut letters = word.chars();
    if !letters.next().is_some_and(char::is_uppercase) {
        return None;
    }
    if letters.any(|letter| letter.is_uppercase() || letter.is_numeric()) || word.contains(CALL_MARK) {
        return None;
    }
    Some(word.trim_end_matches(is_punctuation).to_owned())
}

// `unicode.IsPunct` de Go en ASCII: sin los símbolos (`$`, `+`, `<`, `^`, `|`...).
const fn is_punctuation(letter: char) -> bool {
    matches!(
        letter,
        '!' | '"'
            | '#'
            | '%'
            | '&'
            | '\''
            | '('
            | ')'
            | '*'
            | ','
            | '-'
            | '.'
            | '/'
            | ':'
            | ';'
            | '?'
            | '@'
            | '['
            | '\\'
            | ']'
            | '_'
            | '{'
            | '}'
    )
}

// `errors.New` o `fmt.Errorf` con el nombre con que se importaron.
fn constructor(call: Node<'_>, imports: &HashMap<String, String>, source: &[u8]) -> Option<String> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    if function.kind() != SELECTOR {
        return None;
    }
    let package = function.child_by_field_name(OPERAND_FIELD)?;
    let method = function.child_by_field_name(FIELD_FIELD)?;
    if package.kind() != IDENTIFIER {
        return None;
    }
    let path = imports.get(package.utf8_text(source).ok()?)?;
    let method = method.utf8_text(source).ok()?;
    ERROR_CONSTRUCTORS
        .iter()
        .any(|(constructor_path, name)| path == constructor_path && *name == method)
        .then(|| format!("{path}.{method}"))
}

fn imported_packages(root: Node<'_>, source: &[u8]) -> HashMap<String, String> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == IMPORT_SPEC)
        .filter_map(|spec| {
            let path = spec
                .child_by_field_name(PATH_FIELD)?
                .utf8_text(source)
                .ok()?
                .trim_matches(QUOTE)
                .to_owned();
            let name = spec.child_by_field_name(NAME_FIELD).map_or_else(
                || path.rsplit(PATH_SEPARATOR).next().unwrap_or(&path).to_owned(),
                |name| name.utf8_text(source).unwrap_or_default().to_owned(),
            );
            Some((name, path))
        })
        .collect()
}

// Los nombres que staticcheck exime: las funciones, tipos y variables del paquete que declara el
// archivo y sus métodos, también como `Tipo.Metodo`. Las constantes no: `Invalid` de terraform es una
// constante y golangci-lint avisa igual.
fn declared_names(root: Node<'_>, source: &[u8]) -> Vec<String> {
    let text = |node: Node<'_>| node.utf8_text(source).unwrap_or_default().to_owned();
    let mut names = Vec::new();
    for declaration in named_children(root) {
        match declaration.kind() {
            FUNCTION => names.extend(declaration.child_by_field_name(NAME_FIELD).map(text)),
            METHOD => {
                let Some(name) = declaration.child_by_field_name(NAME_FIELD).map(text) else {
                    continue;
                };
                if let Some(receiver) = receiver_type(declaration, source) {
                    names.push(format!("{receiver}.{name}"));
                }
                names.push(name);
            }
            TYPE_DECLARATION | VAR_DECLARATION => names.extend(
                descendants(declaration)
                    .into_iter()
                    .filter(|spec| DECLARED_SPECS.contains(&spec.kind()))
                    .flat_map(spec_names)
                    .map(text),
            ),
            _ => {}
        }
    }
    names
}

// `func (s *Store[T]) Load()`: `Store`.
fn receiver_type(method: Node<'_>, source: &[u8]) -> Option<String> {
    let parameter = method
        .child_by_field_name(RECEIVER_FIELD)
        .and_then(|list| named_children(list).into_iter().next())?;
    let mut typ = parameter.child_by_field_name(TYPE_FIELD)?;
    while let Some(inner) = match typ.kind() {
        POINTER_TYPE | PARENTHESIZED_TYPE => named_children(typ).into_iter().next(),
        GENERIC_TYPE => typ.child_by_field_name(TYPE_FIELD),
        _ => None,
    } {
        typ = inner;
    }
    typ.utf8_text(source).ok().map(str::to_owned)
}

fn spec_names(spec: Node<'_>) -> Vec<Node<'_>> {
    children_with_fields(spec)
        .into_iter()
        .filter(|(field, _)| *field == Some(NAME_FIELD))
        .map(|(_, name)| name)
        .collect()
}
