mod eradicate;
mod gocritic;
mod python_tokens;

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use tree_sitter::{Node, Parser, Tree};

use crate::profile::CommentedCodeStyle;
use crate::syntax::one_based;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentedCode {
    pub line: usize,
    pub column: usize,
    pub text: String,
}

pub fn find_commented_code(root: Node<'_>, source: &[u8], style: CommentedCodeStyle) -> Vec<CommentedCode> {
    match style {
        CommentedCodeStyle::Eradicate => eradicate::reported(root, source),
        CommentedCodeStyle::Gocritic => gocritic::reported(root, source),
        CommentedCodeStyle::Unmeasured => Vec::new(),
    }
}

fn at(node: Node<'_>, source: &[u8]) -> CommentedCode {
    let start = node.start_position();
    CommentedCode {
        line: one_based(start.row),
        column: one_based(start.column),
        text: node.utf8_text(source).unwrap_or_default().trim().to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SnippetLanguage {
    Python,
    Go,
}

thread_local! {
    static SNIPPET_PARSERS: RefCell<HashMap<SnippetLanguage, Parser>> = RefCell::new(HashMap::new());
}

// Cada comentario candidato se compila aparte; crear un parser por comentario costaba más que
// analizar el archivo entero.
fn parse_snippet(language: SnippetLanguage, text: &str) -> Option<Tree> {
    SNIPPET_PARSERS.with(|parsers| {
        let mut parsers = parsers.borrow_mut();
        let parser = match parsers.entry(language) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let mut parser = Parser::new();
                let grammar = match language {
                    SnippetLanguage::Python => tree_sitter_python::LANGUAGE,
                    SnippetLanguage::Go => tree_sitter_go::LANGUAGE,
                };
                parser.set_language(&grammar.into()).ok()?;
                entry.insert(parser)
            }
        };
        parser.parse(text, None)
    })
}

#[cfg(test)]
mod tests;
