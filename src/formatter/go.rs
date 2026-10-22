mod ast;
mod build_lines;
mod convert;
mod imports;
mod nodes;
mod position;
mod printer;
mod token;

use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::{Parser, Tree};

use super::Unparsable;
use crate::language::Language;
use position::LineTable;

static NEW_CALL: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\bnew(\s*\()").ok());
const NEW_ALIAS: &str = "nev$1";

// Formatea un archivo Go igual que `gofmt` sin opciones: se parsea con tree-sitter, se arma el
// mismo AST que go/parser y se imprime con un port de go/printer.
pub fn format(source: &str) -> Result<String, Unparsable> {
    let tree = parse(source).ok_or(Unparsable)?;
    let lines = LineTable::new(source.as_bytes());
    let mut file = convert::file(tree.root_node(), source, &lines).ok_or(Unparsable)?;
    imports::sort(&mut file, &lines);
    String::from_utf8(nodes::file(&file, &lines)).map_err(|_| Unparsable)
}

// tree-sitter-go todavía no conoce `new(expresión)` de Go 1.26: si el árbol tiene errores se vuelve
// a parsear con `new` cambiado por un identificador del mismo largo, que lo lee como una llamada
// común. Las posiciones no se mueven y el texto se sigue leyendo del fuente original.
fn parse(source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&Language::Go.grammar()?).ok()?;
    let tree = parser.parse(source, None)?;
    if !tree.root_node().has_error() {
        return Some(tree);
    }
    let renamed = NEW_CALL.as_ref()?.replace_all(source, NEW_ALIAS);
    if renamed == source {
        return None;
    }
    let renamed_tree = parser.parse(renamed.as_ref(), None)?;
    (!renamed_tree.root_node().has_error()).then_some(renamed_tree)
}

#[cfg(test)]
mod tests;
