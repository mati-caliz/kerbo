mod ambiguity;
mod ast;
mod build_lines;
mod convert;
mod imports;
mod nodes;
mod position;
mod printer;
mod rejected;
mod token;

use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::{Parser, Tree};

use super::{FIRST_LINE, NO_TREE, Unparsable, line_of, syntax_error};
use crate::language::Language;
use position::LineTable;

static NEW_CALL: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\bnew(\s*\()").ok());
const NEW_ALIAS: &str = "nev$1";
const NEWLINE: char = '\n';

// Formatea un archivo Go igual que `gofmt` sin opciones: se parsea con tree-sitter, se arma el
// mismo AST que go/parser y se imprime con un port de go/printer.
pub fn format(source: &str) -> Result<String, Unparsable> {
    if source.ends_with(NEWLINE) {
        format_terminated(source)
    } else {
        format_terminated(&format!("{source}{NEWLINE}"))
    }
}

// Con el último renglón sin terminar, tree-sitter 0.27 marca error en un archivo que cierra con `}`
// (y no deja ningún nodo de error en el árbol): por eso el fuente llega siempre con su salto final.
fn format_terminated(source: &str) -> Result<String, Unparsable> {
    let tree = parse(source)?;
    if let Some(rejected) = rejected::first_rejected(tree.root_node()) {
        return Err(Unparsable::SyntaxError {
            line: line_of(rejected),
        });
    }
    if let Some(misread) = ambiguity::first_misread(tree.root_node()) {
        return Err(Unparsable::ReadsUnlikeGo {
            line: line_of(misread),
        });
    }
    let mut lines = LineTable::new(source.as_bytes());
    let mut file =
        convert::file(tree.root_node(), source, &lines).map_err(|unsupported| Unparsable::Unsupported {
            line: line_of(unsupported),
        })?;
    imports::sort(&mut file, &mut lines);
    String::from_utf8(nodes::file(&file, &lines)).map_err(|_| Unparsable::Unsupported { line: FIRST_LINE })
}

// tree-sitter-go todavía no conoce `new(expresión)` de Go 1.26: si el árbol tiene errores se vuelve
// a parsear con `new` cambiado por un identificador del mismo largo, que lo lee como una llamada
// común. Las posiciones no se mueven y el texto se sigue leyendo del fuente original.
fn parse(source: &str) -> Result<Tree, Unparsable> {
    let mut parser = Parser::new();
    let grammar = Language::Go.grammar().ok_or(NO_TREE)?;
    parser.set_language(&grammar).map_err(|_| NO_TREE)?;
    let tree = parser.parse(source, None).ok_or(NO_TREE)?;
    if !tree.root_node().has_error() {
        return Ok(tree);
    }
    let renamed = NEW_CALL.as_ref().ok_or(NO_TREE)?.replace_all(source, NEW_ALIAS);
    if renamed == source {
        return Err(syntax_error(&tree));
    }
    let renamed_tree = parser.parse(renamed.as_ref(), None).ok_or(NO_TREE)?;
    if renamed_tree.root_node().has_error() {
        return Err(syntax_error(&renamed_tree));
    }
    Ok(renamed_tree)
}

#[cfg(test)]
mod tests;
