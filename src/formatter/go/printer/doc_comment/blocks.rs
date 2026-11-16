use super::spans::{Span, SpanKind, list_marker, parse_spans, unindent};
use super::text::{is_scheme, linked_text};

const HEADING_PREFIX: &str = "# ";
const CODE_INDENT: char = '\t';
const BULLET: &str = " - ";
const NUMBER_SUFFIX: &str = ". ";
const ITEM_INDENT: &str = "    ";
const LINK_TEXT_OPEN: char = '[';
const LINK_TEXT_CLOSE: &str = "]:";
const LINK_URL_SEPARATORS: [char; 2] = [' ', '\t'];
const SCHEME_SEPARATOR: &str = "://";

#[derive(Debug)]
pub struct LinkDefinition {
    pub text: String,
    pub url: String,
    pub used: bool,
}

#[derive(Debug)]
struct ListItem {
    number: String,
    paragraphs: Vec<String>,
}

#[derive(Debug)]
struct List {
    items: Vec<ListItem>,
    blank_before: bool,
    blank_between: bool,
}

impl List {
    fn is_loose(&self) -> bool {
        self.blank_between || self.items.iter().any(|item| item.paragraphs.len() != 1)
    }
}

#[derive(Debug)]
enum Block {
    Paragraph(String),
    Heading(String),
    Code(Vec<String>),
    List(List),
}

impl Block {
    fn needs_blank_before(&self) -> bool {
        match self {
            Self::List(list) => list.blank_before || list.is_loose(),
            Self::Paragraph(_) | Self::Heading(_) | Self::Code(_) => true,
        }
    }
}

#[derive(Debug, Default)]
struct Doc {
    content: Vec<Block>,
    links: Vec<LinkDefinition>,
}

fn parse_link(line: &str) -> Option<LinkDefinition> {
    let (text, after) = line.strip_prefix(LINK_TEXT_OPEN)?.split_once(LINK_TEXT_CLOSE)?;
    let url = after
        .strip_prefix(LINK_URL_SEPARATORS)
        .filter(|rest| !rest.is_empty())?
        .trim();
    let (scheme, _) = url.split_once(SCHEME_SEPARATOR)?;
    is_scheme(scheme).then(|| LinkDefinition {
        text: text.to_owned(),
        url: url.to_owned(),
        used: false,
    })
}

impl Doc {
    // Un párrafo hecho sólo de definiciones de enlace (`[texto]: url`) no es texto: las
    // definiciones se guardan aparte y se imprimen al final.
    fn paragraph(&mut self, lines: &[&str]) -> Option<String> {
        let definitions: Option<Vec<LinkDefinition>> = lines.iter().map(|line| parse_link(line)).collect();
        match definitions {
            Some(definitions) => {
                self.links.extend(definitions);
                None
            }
            None => Some(lines.join("\n")),
        }
    }

    fn close_item(&mut self, list: &mut List, text: &mut Vec<&str>) {
        let paragraph = self.paragraph(text);
        text.clear();
        if let Some(item) = list.items.last_mut() {
            item.paragraphs.extend(paragraph);
        }
    }

    fn list(&mut self, lines: &[&str], blank_before: bool) -> List {
        let numbered = lines
            .first()
            .and_then(|line| list_marker(line))
            .is_some_and(|(number, _)| !number.is_empty());
        let mut list = List {
            items: Vec::new(),
            blank_before,
            blank_between: false,
        };
        let mut text: Vec<&str> = Vec::new();
        for line in lines {
            let marker = list_marker(line).filter(|(number, _)| number.is_empty() != numbered);
            let content = marker.map_or(*line, |(_, rest)| rest).trim();
            if let Some((number, _)) = marker {
                self.close_item(&mut list, &mut text);
                list.items.push(ListItem {
                    number: number.to_owned(),
                    paragraphs: Vec::new(),
                });
            }
            if content.is_empty() {
                list.blank_between = true;
                self.close_item(&mut list, &mut text);
            } else {
                text.push(content);
            }
        }
        self.close_item(&mut list, &mut text);
        list
    }

    fn block(&mut self, lines: &[&str], span: Span, previous_end: usize) -> Option<Block> {
        let span_lines = lines.get(span.start..span.end).unwrap_or_default();
        let first = span_lines.first().copied().unwrap_or_default();
        match span.kind {
            SpanKind::List => Some(Block::List(self.list(span_lines, previous_end < span.start))),
            SpanKind::Code => Some(Block::Code(
                unindent(span_lines).into_iter().map(str::to_owned).collect(),
            )),
            SpanKind::OldHeading => Some(Block::Heading(first.trim().to_owned())),
            SpanKind::Heading => Some(Block::Heading(
                first.get(1..).unwrap_or_default().trim().to_owned(),
            )),
            SpanKind::Paragraph => self.paragraph(span_lines).map(Block::Paragraph),
        }
    }

    fn parse(text: &str) -> Self {
        let split: Vec<&str> = text.split('\n').collect();
        let lines = unindent(&split);
        let mut doc = Self::default();
        let mut previous_end = 0;
        for span in parse_spans(&lines) {
            let block = doc.block(&lines, span, previous_end);
            doc.content.extend(block);
            previous_end = span.end;
        }
        doc
    }
}

fn push_indented(out: &mut String, indent: &str, text: &str) {
    out.push_str(&text.replace('\n', &format!("\n{indent}")));
}

fn print_list(out: &mut String, list: &List, links: &mut [LinkDefinition]) {
    let loose = list.is_loose();
    for (index, item) in list.items.iter().enumerate() {
        if index > 0 && loose {
            out.push('\n');
        }
        out.push(' ');
        if item.number.is_empty() {
            out.push_str(BULLET);
        } else {
            out.push_str(&item.number);
            out.push_str(NUMBER_SUFFIX);
        }
        for (position, paragraph) in item.paragraphs.iter().enumerate() {
            if position > 0 {
                out.push('\n');
                out.push_str(ITEM_INDENT);
            }
            push_indented(out, ITEM_INDENT, &linked_text(paragraph, links));
            out.push('\n');
        }
    }
}

fn print_block(out: &mut String, block: &Block, links: &mut [LinkDefinition]) {
    match block {
        Block::Paragraph(text) => {
            out.push_str(&linked_text(text, links));
            out.push('\n');
        }
        Block::Heading(text) => {
            out.push_str(HEADING_PREFIX);
            out.push_str(text);
            out.push('\n');
        }
        Block::Code(code) => {
            for line in code {
                if !line.is_empty() {
                    out.push(CODE_INDENT);
                    out.push_str(line);
                }
                out.push('\n');
            }
        }
        Block::List(list) => print_list(out, list, links),
    }
}

// Las definiciones de enlace van al final: primero las usadas y después, en otro bloque, las que
// ningún texto nombra. Si el texto de una definición se repite, vale la primera.
fn print_links(out: &mut String, links: &[LinkDefinition]) {
    for used in [true, false] {
        let mut first = true;
        for definition in links.iter().filter(|definition| definition.used == used) {
            if first {
                out.push('\n');
                first = false;
            }
            out.push(LINK_TEXT_OPEN);
            out.push_str(&definition.text);
            out.push_str(LINK_TEXT_CLOSE);
            out.push(' ');
            out.push_str(&definition.url);
            out.push('\n');
        }
    }
}

// `Parser.Parse` y `Printer.Comment` de go/doc/comment: el texto de un comentario de documentación,
// sin los marcadores, en su forma canónica.
pub fn reformat(text: &str) -> String {
    let mut doc = Doc::parse(text);
    let mut out = String::new();
    for (index, block) in doc.content.iter().enumerate() {
        if index > 0 && block.needs_blank_before() {
            out.push('\n');
        }
        print_block(&mut out, block, &mut doc.links);
    }
    print_links(&mut out, &doc.links);
    out
}
