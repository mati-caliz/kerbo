// Calca los errores del parser de Javadoc de Checkstyle 14 (`JavadocCommentsLexer.g4` y
// `JavadocCommentsParser.g4`) en lo que toca al HTML: con un error, `JavadocMethod` y `JavadocType`
// avisan el error en su línea y no revisan las etiquetas de ese Javadoc. Gana el primer error del
// parser (corta ahí) y, si no hay, el último del lexer.

use super::java_javadoc_lexer::{Kind, Lexer, Token};

const VOID_TAGS: [&str; 13] = [
    "area", "base", "basefont", "br", "col", "frame", "hr", "img", "input", "isindex", "link", "meta",
    "param",
];
const UNEXPECTED_END_TAG: &str = "no viable alternative at input '</'";
const UNEXPECTED_TOKEN: &str = "mismatched input";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub row: usize,
    pub detail: String,
}

pub fn parse_error(comment: &str, first_row: usize) -> Option<ParseError> {
    let body = comment.strip_prefix("/**").unwrap_or(comment);
    let body = body.strip_suffix("*/").unwrap_or(body);
    let mut lexer = Lexer::new(body, first_row);
    lexer.run();
    let visible: Vec<Token> = lexer
        .tokens
        .iter()
        .filter(|token| token.kind != Kind::Blank)
        .cloned()
        .collect();
    let unclosed = unclosed_names(&lexer.tokens);
    let mut parser = Parser {
        tokens: &visible,
        position: 0,
        unclosed: &unclosed,
    };
    parser.content(false).err().or(lexer.last_error)
}

// `JavadocCommentsLexerUtil.getUnclosedTagNameTokens`: cada cierre, del último al primero, se lleva la
// apertura del mismo nombre más reciente anterior a él; las que quedan no se cierran. Un `<p/>`
// también cuenta como apertura.
fn unclosed_names(tokens: &[Token]) -> Vec<usize> {
    let mut opens: Vec<(usize, String)> = Vec::new();
    let mut closes: Vec<(usize, String)> = Vec::new();
    let mut visible_index = 0;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == Kind::Name {
            let after_slash = index
                .checked_sub(1)
                .and_then(|previous| tokens.get(previous))
                .is_some_and(|previous| previous.kind == Kind::Slash);
            let entry = (visible_index, token.text.to_lowercase());
            if after_slash {
                closes.push(entry);
            } else {
                opens.push(entry);
            }
        }
        if token.kind != Kind::Blank {
            visible_index += 1;
        }
    }
    for (closing_index, closing_name) in closes.iter().rev() {
        if let Some(found) = opens
            .iter()
            .rposition(|(open_index, open_name)| open_name == closing_name && open_index < closing_index)
        {
            opens.remove(found);
        }
    }
    opens.into_iter().map(|(index, _)| index).collect()
}

struct Parser<'tokens> {
    tokens: &'tokens [Token],
    position: usize,
    unclosed: &'tokens [usize],
}

impl Parser<'_> {
    fn kind_at(&self, offset: usize) -> Option<Kind> {
        self.tokens.get(self.position + offset).map(|token| token.kind)
    }

    fn error_here(&self, detail: &str) -> ParseError {
        let row = self
            .tokens
            .get(self.position)
            .or_else(|| self.tokens.last())
            .map_or(0, |token| token.row);
        ParseError {
            row,
            detail: detail.to_owned(),
        }
    }

    // `mainDescription`, `description` y `htmlContent`: texto, etiquetas en línea, comentarios y
    // elementos; dentro de un elemento cerrado termina en el primer `</`.
    fn content(&mut self, inside_element: bool) -> Result<(), ParseError> {
        while let Some(kind) = self.kind_at(0) {
            if self.content_ends(kind, inside_element)? {
                return Ok(());
            }
        }
        if inside_element {
            Err(self.error_here(UNEXPECTED_TOKEN))
        } else {
            Ok(())
        }
    }

    fn content_ends(&mut self, kind: Kind, inside_element: bool) -> Result<bool, ParseError> {
        let next = self.kind_at(1);
        match kind {
            Kind::Text | Kind::Inline | Kind::Comment => {
                self.position += 1;
                Ok(false)
            }
            Kind::Open if next == Some(Kind::Slash) && inside_element => Ok(true),
            Kind::Open if next == Some(Kind::Slash) => Err(self.error_here(UNEXPECTED_END_TAG)),
            Kind::Open if next == Some(Kind::Name) => self.element().map(|()| false),
            Kind::Open
            | Kind::Slash
            | Kind::Name
            | Kind::AttributeName
            | Kind::Equals
            | Kind::Value
            | Kind::Close
            | Kind::SlashClose
            | Kind::Blank => Err(self.error_here(UNEXPECTED_TOKEN)),
        }
    }

    fn element(&mut self) -> Result<(), ParseError> {
        let name_index = self.position + 1;
        let name = self
            .tokens
            .get(name_index)
            .map(|token| token.text.to_lowercase())
            .unwrap_or_default();
        self.position += 2;
        self.attributes()?;
        match self.kind_at(0) {
            Some(Kind::SlashClose) => {
                self.position += 1;
                Ok(())
            }
            Some(Kind::Close) => {
                self.position += 1;
                self.element_body(&name, name_index)
            }
            _ => Err(self.error_here(UNEXPECTED_TOKEN)),
        }
    }

    fn attributes(&mut self) -> Result<(), ParseError> {
        while self.kind_at(0) == Some(Kind::AttributeName) {
            self.position += 1;
            if self.kind_at(0) != Some(Kind::Equals) {
                continue;
            }
            if self.kind_at(1) != Some(Kind::Value) {
                self.position += 1;
                return Err(self.error_here(UNEXPECTED_TOKEN));
            }
            self.position += 2;
        }
        Ok(())
    }

    // Un elemento vacío no tiene cuerpo; uno que no se cierra sólo lleva texto y etiquetas en línea;
    // el resto, contenido hasta su `</...>`.
    fn element_body(&mut self, name: &str, name_index: usize) -> Result<(), ParseError> {
        if VOID_TAGS.contains(&name) {
            return Ok(());
        }
        if self.unclosed.contains(&name_index) {
            while matches!(self.kind_at(0), Some(Kind::Text | Kind::Inline)) {
                self.position += 1;
            }
            return Ok(());
        }
        self.content(true)?;
        self.end_tag()
    }

    fn end_tag(&mut self) -> Result<(), ParseError> {
        let expected = [Kind::Open, Kind::Slash, Kind::Name, Kind::Close];
        let matched = expected
            .iter()
            .enumerate()
            .take_while(|(offset, kind)| self.kind_at(*offset) == Some(**kind))
            .count();
        self.position += matched;
        if matched == expected.len() {
            Ok(())
        } else {
            Err(self.error_here(UNEXPECTED_TOKEN))
        }
    }
}
