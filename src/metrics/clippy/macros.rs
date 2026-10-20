use tree_sitter::{Node, Tree};

use crate::syntax::{SnippetLanguage, children_with_fields, parse_snippet};

const MACRO_FIELD: &str = "macro";
const BODY_FIELD: &str = "body";
const TOKEN_TREE: &str = "token_tree";
const SEPARATORS: &[&str] = &[",", ";"];
const GUARD: &str = "if";
const PATH_SEPARATOR: &str = "::";
const WRAPPER_PREFIX: &str = "fn kerbo() { (";
const WRAPPER_SUFFIX: &str = "); }";
const ASSERTIONS: &[&str] = &["assert", "assert_eq", "assert_ne"];
const DEBUG_ASSERTIONS: &[&str] = &["debug_assert", "debug_assert_eq", "debug_assert_ne"];
const MATCHES: &str = "matches";
const LOG_LEVELS: &[&str] = &["log", "trace", "debug", "info", "warn", "error"];
const LOGGING_CRATES: &[&str] = &["log::", "tracing::"];
const LOG_CONDITIONS: usize = 1;
const READY: &str = "ready";
const ASSERTION_CONDITIONS: usize = 1;
const DEBUG_ASSERTION_CONDITIONS: usize = 2;
const MATCHES_ARMS: usize = 1;
const READY_ARMS: usize = 1;
const READY_RETURNS: usize = 1;

#[derive(Debug)]
pub(super) struct Argument {
    pub tree: Tree,
    pub text: String,
    pub first_row: usize,
}

impl Argument {
    pub(super) fn expression(&self) -> Option<Node<'_>> {
        let function = self.tree.root_node().named_child(0)?;
        let statement = function.child_by_field_name(BODY_FIELD)?.named_child(0)?;
        statement.named_child(0)
    }
}

#[derive(Debug, Default)]
pub(super) struct Expansion {
    pub added: usize,
    pub returns: usize,
    pub arguments: Vec<Argument>,
}

// Lo que suma cada macro de la biblioteca estándar una vez expandida, que es lo que mira clippy:
// assert! es un `if`, la versión debug lo envuelve en otro `if cfg!(..)`, matches! es un `match`
// de dos brazos y ready! uno con un `return`. Las de `log` (y las de `tracing` sin su puente a
// `log`) chequean el nivel con un `if`, pero sólo se cuentan cuando el archivo las toma de esos
// crates: muchos crates tienen su propio `debug!`. Los argumentos se compilan aparte y se cuentan
// como código propio; de una macro desconocida sólo se cuentan los argumentos que son expresiones.
pub(super) fn expand(invocation: Node<'_>, source: &[u8]) -> Expansion {
    let name = macro_name(invocation, source);
    let pieces = argument_pieces(invocation);
    let parsed = |tokens: &[Node<'_>]| parse_argument(tokens, source);
    if name == MATCHES {
        let scrutinee = pieces.first().map(Vec::as_slice);
        let pattern: Vec<Node<'_>> = pieces.get(1..).unwrap_or_default().concat();
        let guard = pattern
            .iter()
            .position(|token| token.kind() == GUARD)
            .and_then(|position| pattern.get(position.saturating_add(1)..));
        return Expansion {
            added: MATCHES_ARMS.saturating_add(usize::from(guard.is_some())),
            returns: 0,
            arguments: [scrutinee, guard]
                .into_iter()
                .flatten()
                .filter_map(parsed)
                .collect(),
        };
    }
    let (added, returns) = if ASSERTIONS.contains(&name) {
        (ASSERTION_CONDITIONS, 0)
    } else if DEBUG_ASSERTIONS.contains(&name) {
        (DEBUG_ASSERTION_CONDITIONS, 0)
    } else if name == READY {
        (READY_ARMS, READY_RETURNS)
    } else if LOG_LEVELS.contains(&name) && uses_logging_crate(source) {
        (LOG_CONDITIONS, 0)
    } else {
        (0, 0)
    };
    Expansion {
        added,
        returns,
        arguments: pieces.iter().filter_map(|piece| parsed(piece)).collect(),
    }
}

fn macro_name<'source>(invocation: Node<'_>, source: &'source [u8]) -> &'source str {
    invocation
        .child_by_field_name(MACRO_FIELD)
        .and_then(|name| name.utf8_text(source).ok())
        .and_then(|path| path.rsplit(PATH_SEPARATOR).next())
        .unwrap_or_default()
}

fn uses_logging_crate(source: &[u8]) -> bool {
    std::str::from_utf8(source).is_ok_and(|text| LOGGING_CRATES.iter().any(|path| text.contains(path)))
}

fn argument_pieces(invocation: Node<'_>) -> Vec<Vec<Node<'_>>> {
    let Some(tree) = children_with_fields(invocation)
        .into_iter()
        .map(|(_, child)| child)
        .find(|child| child.kind() == TOKEN_TREE)
    else {
        return Vec::new();
    };
    let tokens: Vec<Node<'_>> = children_with_fields(tree)
        .into_iter()
        .map(|(_, child)| child)
        .collect();
    let inner = tokens.get(1..tokens.len().saturating_sub(1)).unwrap_or_default();
    inner
        .split(|token| SEPARATORS.contains(&token.kind()))
        .filter(|piece| !piece.is_empty())
        .map(<[Node<'_>]>::to_vec)
        .collect()
}

fn parse_argument(tokens: &[Node<'_>], source: &[u8]) -> Option<Argument> {
    let start = tokens.first()?.start_byte();
    let end = tokens.last()?.end_byte();
    let text = std::str::from_utf8(source.get(start..end)?).ok()?;
    let wrapped = format!("{WRAPPER_PREFIX}{text}{WRAPPER_SUFFIX}");
    let tree = parse_snippet(SnippetLanguage::Rust, &wrapped)?;
    let first_row = tokens.first()?.start_position().row;
    (!tree.root_node().has_error()).then_some(Argument {
        tree,
        text: wrapped,
        first_row,
    })
}
