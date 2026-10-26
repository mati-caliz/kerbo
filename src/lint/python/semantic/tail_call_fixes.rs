use tree_sitter::Node;

use super::builtin_calls::arguments;
use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::integer_calls::is_integer_literal;
use super::model::Model;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{
    CONCATENATED_STRING, FALSE, MODULE, NOT_OPERATOR, TRUE, text, unparenthesized,
};
use crate::lint::python::codegen;
use crate::lint::python::fix_edits::{commented_unsafe, has_comment_within};

const MATH: &str = "math";
const LOGGING: &str = "logging";
const GET_LOGGER: &str = "getLogger";
const LIST_SPLAT: &str = "list_splat";
const RADIX_PREFIXES: [&str; 6] = ["0b", "0B", "0x", "0X", "0o", "0O"];
const NUMBERS: [&str; 2] = ["integer", "float"];
const WITHOUT_PARENTHESES: [&str; 15] = [
    "call",
    "dictionary",
    "set",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "true",
    "false",
    "none",
    "ellipsis",
    "attribute",
    "subscript",
    "string",
    "parenthesized_expression",
    "dictionary_splat",
];
const TRANSPARENT: [&str; 3] = ["parenthesized_expression", "argument_list", "keyword_argument"];
const COMPREHENSION_CLAUSES: [&str; 2] = ["for_in_clause", "if_clause"];
const NON_EXPRESSION_PARENTS: [&str; 7] = [
    "assignment",
    "augmented_assignment",
    "decorator",
    "default_parameter",
    "typed_default_parameter",
    "with_item",
    "block",
];
const STATEMENT_SUFFIXES: [&str; 2] = ["_statement", "_clause"];

// FURB161: `bin(x).count("1")` pasa a `x.bit_count()`, entre paréntesis salvo donde ruff sabe que no
// hacen falta. Seguro sólo si `x` es un entero o un booleano escrito.
pub fn bit_count_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Fix {
    let argument = unparenthesized(argument);
    let written = text(argument, source);
    let parenthesize = if NUMBERS.contains(&argument.kind()) {
        !RADIX_PREFIXES.iter().any(|prefix| written.starts_with(prefix))
    } else {
        argument.kind() == CONCATENATED_STRING || !WITHOUT_PARENTHESES.contains(&argument.kind())
    };
    let replacement = if parenthesize {
        format!("({written}).bit_count()")
    } else {
        format!("{written}.bit_count()")
    };
    let is_integer =
        is_integer_literal(argument, source) || matches!(argument.kind(), TRUE | FALSE | NOT_OPERATOR);
    let fix = Fix::replace(call, &replacement, source);
    if is_integer { fix } else { fix.marked_unsafe() }
}

// FURB168, el `replace_with_identity_check` de ruff: `x is None`, entre paréntesis si la llamada está
// adentro de otra expresión.
pub fn identity_check_fix(call: Node<'_>, checked: Node<'_>, source: &[u8]) -> Fix {
    let comparison = format!(
        "{} is None",
        codegen::unparse(checked, codegen::CMP.saturating_add(1), source)
    );
    let replacement = if is_inside_expression(call) {
        format!("({comparison})")
    } else {
        comparison
    };
    commented_unsafe(Fix::replace(call, &replacement, source), call, source)
}

// FURB163: `math.log(x, 2)` pasa a `math.log2(x)`. Inseguro con base 2 o 10 (cambia la precisión), con
// `*args` o con comentarios adentro.
pub fn log_base_fix(model: &Model<'_>, call: Node<'_>, function: &str, source: &[u8]) -> Option<Fix> {
    let argument = *arguments(call).positional.first()?;
    let (binding, import) = get_or_import_member(model, source, call, import_module(MATH, function))?;
    let replacement = TextEdit {
        start: TextPoint::from_point(call.start_position()),
        end: TextPoint::from_point(call.end_position()),
        replacement: format!("{binding}({})", text(argument, source)),
    };
    let fix = Fix::from_edits(source, vec![replacement, import]);
    let risky = function != MATH_LOG_FUNCTION
        || argument.kind() == LIST_SPLAT
        || has_comment_within(call, call.start_byte(), call.end_byte());
    Some(if risky { fix.marked_unsafe() } else { fix })
}

pub const MATH_LOG_FUNCTION: &str = "log";

// LOG001: `logging.Logger(...)` pasa a `logging.getLogger(...)`; siempre inseguro, como en ruff.
pub fn get_logger_fix(model: &Model<'_>, function: Node<'_>, source: &[u8]) -> Option<Fix> {
    let (binding, import) =
        get_or_import_member(model, source, function, import_module(LOGGING, GET_LOGGER))?;
    let reference = TextEdit {
        start: TextPoint::from_point(function.start_position()),
        end: TextPoint::from_point(function.end_position()),
        replacement: binding,
    };
    Some(Fix::from_edits(source, vec![reference, import]).marked_unsafe())
}

const fn import_module<'request>(module: &'request str, member: &'request str) -> ImportRequest<'request> {
    ImportRequest {
        module,
        member,
        style: ImportStyle::Import,
    }
}

// El `current_expression_parent` de ruff: si la expresión cuelga de otra, sin contar paréntesis ni
// argumentos, que en el AST de ruff no son nodos.
pub fn is_inside_expression(node: Node<'_>) -> bool {
    expression_parent(node).is_some()
}

pub fn expression_parent(node: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
        .find(|ancestor| !TRANSPARENT.contains(&ancestor.kind()))
        .filter(|parent| {
            let kind = parent.kind();
            COMPREHENSION_CLAUSES.contains(&kind)
                || !(NON_EXPRESSION_PARENTS.contains(&kind)
                    || kind == MODULE
                    || STATEMENT_SUFFIXES.iter().any(|suffix| kind.ends_with(suffix)))
        })
}
