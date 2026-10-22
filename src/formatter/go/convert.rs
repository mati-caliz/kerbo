mod comments;
mod declarations;
mod expressions;
mod statements;
mod types;

use tree_sitter::Node;

use super::ast::{BasicLit, File, Ident};
use super::position::{LineTable, Pos};
use super::token::Token;
use comments::Comments;

const COMMENT: &str = "comment";
const CARRIAGE_RETURN: char = '\r';
const PACKAGE_CLAUSE: &str = "package_clause";

#[derive(Debug, Clone, Copy)]
pub struct Missing;

#[derive(Debug)]
pub struct Converter<'source> {
    source: &'source str,
    comments: &'source Comments,
}

pub fn file(root: Node<'_>, source: &str, lines: &LineTable) -> Option<File> {
    let comments = comments::scan(root, source, lines);
    let converter = Converter {
        source,
        comments: &comments,
    };
    let mut nodes = named(root).into_iter();
    let package = nodes.next().filter(|node| node.kind() == PACKAGE_CLAUSE)?;
    let name = package.named_child(0).map(|node| converter.ident(node))?;
    let decls = nodes
        .map(|node| declarations::top_level(&converter, node))
        .collect::<Option<Vec<_>>>()?;
    Some(File {
        package: start(package),
        name,
        decls,
        comments: comments.into_groups(),
    })
}

pub fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

pub fn named(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

pub fn named_field<'tree>(node: Node<'tree>, field: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor)
        .filter(|child| child.is_named() && child.kind() != COMMENT)
        .collect()
}

pub fn start(node: Node<'_>) -> Pos {
    Pos::at(node.start_byte())
}

pub fn token(node: Node<'_>, text: &str) -> Pos {
    children(node)
        .into_iter()
        .find(|child| !child.is_named() && child.kind() == text)
        .map_or(Pos::NONE, start)
}

pub fn last_token(node: Node<'_>, text: &str) -> Pos {
    children(node)
        .into_iter()
        .rev()
        .find(|child| !child.is_named() && child.kind() == text)
        .map_or(Pos::NONE, start)
}

impl Converter<'_> {
    pub fn text(&self, node: Node<'_>) -> &str {
        self.source.get(node.byte_range()).unwrap_or_default()
    }

    pub fn ident(&self, node: Node<'_>) -> Ident {
        Ident {
            name: self.text(node).to_owned(),
            pos: start(node),
        }
    }

    pub fn literal(&self, node: Node<'_>, kind: Token) -> BasicLit {
        let text = self.text(node);
        let value = if text.contains(CARRIAGE_RETURN) && kind == Token::String {
            text.replace(CARRIAGE_RETURN, "")
        } else {
            text.to_owned()
        };
        BasicLit {
            kind,
            value,
            pos: start(node),
            end: Pos::at(node.end_byte()),
        }
    }

    pub fn line_comment(&self, node: Node<'_>) -> Option<usize> {
        self.comments.line_comment(node.end_byte())
    }
}
