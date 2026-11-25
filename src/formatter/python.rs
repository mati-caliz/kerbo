mod ast;
mod builders;
mod comments;
mod context;
mod convert;
mod expression;
mod parentheses;
mod statement;
mod string;
mod trivia;

use tree_sitter::{Node, Parser, Tree};

use super::document::{Builder, LineMode, print};
use super::{FIRST_LINE, NO_TREE, Unparsable, line_of, syntax_error};
use crate::language::Language;
use crate::syntax::descendants;
use ast::{NodeId, Other, Span};
use comments::format::leading_node_comments;
use comments::placement::Placer;
use comments::suppression::is_suppression_comment;
use comments::visitor;
use context::{Formatted, Formatter, InterpolatedState, NodeLevel, TopLevelPosition, Unsupported};
use convert::Converter;
use statement::suite::{SuiteKind, fmt_suite};

const COMMENT: &str = "comment";
const BYTE_ORDER_MARK: char = '\u{feff}';
const NEWLINE: char = '\n';
const CARRIAGE_RETURN: char = '\r';
const SUPPRESSION: &str = "comentario que apaga el formateo";
const UNWRITTEN_COMMENT: &str = "comentario en un lugar que todavía no se escribe";
const START_OF_FILE: usize = 0;

fn unsupported_node(node: Node<'_>) -> Unparsable {
    Unparsable::UnsupportedSyntax {
        line: line_of(node),
        syntax: node.kind().to_owned(),
    }
}

fn unsupported_at(source: &str, unsupported: Unsupported) -> Unparsable {
    let before = source.get(..unsupported.offset).unwrap_or_default();
    Unparsable::UnsupportedSyntax {
        line: before.matches(NEWLINE).count().saturating_add(FIRST_LINE),
        syntax: unsupported.syntax.to_owned(),
    }
}

fn fmt_module(f: &mut Formatter<'_>, module: NodeId) -> Formatted {
    let Some(Other::Module { body }) = f.ast.other(module) else {
        return Ok(());
    };
    leading_node_comments(f, module);
    if body.is_empty() {
        if !f.comments.has_leading(module) && trivia::lines_after(f.source, START_OF_FILE) != 0 {
            f.line(LineMode::Empty);
        }
        return Ok(());
    }
    fmt_suite(f, body, SuiteKind::TopLevel)?;
    f.line(LineMode::Hard);
    Ok(())
}

// Formatea un archivo Python igual que `ruff format` sin configurar: se parsea con tree-sitter, se
// arma el mismo AST que el parser de ruff, y de ahí el mismo documento que ruff_python_formatter,
// que se imprime con un port de su impresor. Está en construcción: un archivo con sintaxis que
// todavía no se portó queda como está.
pub fn format(source: &str) -> Result<String, Unparsable> {
    if source.starts_with(BYTE_ORDER_MARK) {
        return Err(Unparsable::Unsupported { line: FIRST_LINE });
    }
    let tree = parse(source)?;
    let root = tree.root_node();
    let comment_ranges = comment_ranges_of(source, root);
    if let Some(suppression) = comment_ranges
        .iter()
        .find(|comment| is_suppression_comment(source.get(comment.start..comment.end).unwrap_or_default()))
    {
        return Err(unsupported_at(
            source,
            Unsupported {
                offset: suppression.start,
                syntax: SUPPRESSION,
            },
        ));
    }
    let mut converter = Converter::new(source);
    let module = convert::module(&mut converter, root).map_err(unsupported_node)?;
    let placer = Placer {
        ast: &converter.ast,
        source,
        comment_ranges: &comment_ranges,
        parenthesized: &converter.parenthesized,
    };
    let comments =
        visitor::collect(&placer, module).map_err(|unsupported| unsupported_at(source, unsupported))?;
    let mut formatter = Formatter {
        builder: Builder::default(),
        source,
        ast: &converter.ast,
        comments: &comments,
        parenthesized: &converter.parenthesized,
        comment_ranges: &comment_ranges,
        node_level: NodeLevel::TopLevel(TopLevelPosition::Other),
        indent_level: 0,
        docstring_quote: None,
        interpolated: InterpolatedState::Outside,
    };
    fmt_module(&mut formatter, module).map_err(|unsupported| unsupported_at(source, unsupported))?;
    if let Some(unwritten) = comments.first_unformatted() {
        return Err(unsupported_at(
            source,
            Unsupported {
                offset: unwritten.start,
                syntax: UNWRITTEN_COMMENT,
            },
        ));
    }
    let printed = print(&formatter.builder.finish())
        .map_err(|_invalid| Unparsable::Unsupported { line: FIRST_LINE })?;
    Ok(trivia::with_the_line_ending_of(source, &printed))
}

// Los comentarios del archivo en orden. tree-sitter deja adentro el `\r` de un salto de línea de
// Windows, que no es parte del comentario.
fn comment_ranges_of(source: &str, root: Node<'_>) -> Vec<Span> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == COMMENT)
        .map(|node| {
            let text = source.get(node.byte_range()).unwrap_or_default();
            let length = text.trim_end_matches(CARRIAGE_RETURN).len();
            Span::new(node.start_byte(), node.start_byte().saturating_add(length))
        })
        .collect()
}

fn parse(source: &str) -> Result<Tree, Unparsable> {
    let mut parser = Parser::new();
    let grammar = Language::Python.grammar().ok_or(NO_TREE)?;
    parser.set_language(&grammar).map_err(|_| NO_TREE)?;
    let tree = parser.parse(source, None).ok_or(NO_TREE)?;
    if tree.root_node().has_error() {
        return Err(syntax_error(&tree));
    }
    Ok(tree)
}

#[cfg(test)]
mod tests;
