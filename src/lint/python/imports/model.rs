use tree_sitter::Node;

use crate::lint::python::ast::text;

const NEWLINE: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';

#[derive(Debug, Clone, Copy)]
pub struct SourceText<'source> {
    pub bytes: &'source [u8],
}

impl<'source> SourceText<'source> {
    // `line_end`: el final de la línea, antes del salto.
    pub fn line_end(self, offset: usize) -> usize {
        self.bytes
            .get(offset..)
            .and_then(|rest| {
                rest.iter()
                    .position(|byte| matches!(*byte, NEWLINE | CARRIAGE_RETURN))
            })
            .map_or(self.bytes.len(), |distance| offset.saturating_add(distance))
    }

    // `full_line_end`: el final de la línea, después del salto.
    pub fn full_line_end(self, offset: usize) -> usize {
        let end = self.line_end(offset);
        match (self.bytes.get(end), self.bytes.get(end.saturating_add(1))) {
            (Some(&CARRIAGE_RETURN), Some(&NEWLINE)) => end.saturating_add(2),
            (Some(_), _) => end.saturating_add(1),
            (None, _) => end,
        }
    }

    pub fn line_start(self, offset: usize) -> usize {
        self.bytes
            .get(..offset)
            .and_then(|before| {
                before
                    .iter()
                    .rposition(|byte| matches!(*byte, NEWLINE | CARRIAGE_RETURN))
            })
            .map_or(0, |newline| newline.saturating_add(1))
    }

    pub fn has_line_break(self, start: usize, end: usize) -> bool {
        self.bytes.get(start..end).is_some_and(|between| {
            between
                .iter()
                .any(|byte| matches!(*byte, NEWLINE | CARRIAGE_RETURN))
        })
    }

    pub fn slice(self, start: usize, end: usize) -> &'source str {
        std::str::from_utf8(self.bytes.get(start..end).unwrap_or_default()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alias<'source> {
    pub name: &'source str,
    pub asname: Option<&'source str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FromModule<'source> {
    pub module: Option<&'source str>,
    pub level: usize,
}

impl FromModule<'_> {
    // `format_import_from`: los puntos del nivel y el módulo.
    pub fn name(self) -> String {
        format!("{}{}", ".".repeat(self.level), self.module.unwrap_or_default())
    }
}

#[derive(Debug, Clone, Default)]
pub struct AliasComments<'source> {
    pub atop: Vec<&'source str>,
    pub inline: Vec<&'source str>,
    pub trailing: Vec<&'source str>,
}

impl AliasComments<'_> {
    pub fn append(&mut self, other: Self) {
        self.atop.extend(other.atop);
        self.inline.extend(other.inline);
        self.trailing.extend(other.trailing);
    }

    pub const fn is_empty(&self) -> bool {
        self.atop.is_empty() && self.inline.is_empty() && self.trailing.is_empty()
    }
}

#[derive(Debug)]
pub struct AnnotatedAlias<'source> {
    pub alias: Alias<'source>,
    pub comments: AliasComments<'source>,
}

#[derive(Debug)]
pub struct FromParts<'source> {
    pub module: FromModule<'source>,
    pub aliases: Vec<AnnotatedAlias<'source>>,
    pub comments: AliasComments<'source>,
    pub trailing_comma: bool,
}

#[derive(Debug)]
pub enum Annotated<'source> {
    Import {
        names: Vec<Alias<'source>>,
        atop: Vec<&'source str>,
        inline: Vec<&'source str>,
    },
    From(FromParts<'source>),
}

// Los comentarios del bloque, que se van repartiendo entre los imports en orden.
pub struct Comments<'tree, 'source> {
    nodes: Vec<Node<'tree>>,
    next: usize,
    text_source: SourceText<'source>,
}

impl<'tree, 'source> Comments<'tree, 'source> {
    pub fn new(nodes: &[Node<'tree>], text_source: SourceText<'source>) -> Self {
        Self {
            nodes: nodes.to_vec(),
            next: 0,
            text_source,
        }
    }

    pub fn take_while_start_before(&mut self, offset: usize) -> Vec<&'source str> {
        self.take_while(|comment| comment.start_byte() < offset)
    }

    // tree-sitter deja el `\r` de un final de línea de Windows adentro del comentario.
    pub fn take_while_end_at_most(&mut self, offset: usize) -> Vec<&'source str> {
        let bytes = self.text_source.bytes;
        self.take_while(|comment| {
            let carriage_return = bytes.get(comment.end_byte().saturating_sub(1)) == Some(&CARRIAGE_RETURN);
            comment.end_byte().saturating_sub(usize::from(carriage_return)) <= offset
        })
    }

    fn take_while(&mut self, accepts: impl Fn(Node<'tree>) -> bool) -> Vec<&'source str> {
        let mut taken = Vec::new();
        while let Some(comment) = self
            .nodes
            .get(self.next)
            .copied()
            .filter(|comment| accepts(*comment))
        {
            taken.push(text(comment, self.text_source.bytes).trim_end_matches('\r'));
            self.next = self.next.saturating_add(1);
        }
        taken
    }
}
