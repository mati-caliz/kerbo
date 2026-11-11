use std::collections::HashMap;

use tree_sitter::Node;

use crate::syntax::named_children;

const PAIR: &str = "pair";
const HASH: &str = "hash";
const ARRAY: &str = "array";
const ARRAY_KINDS: &[&str] = &[ARRAY, "string_array", "symbol_array"];
const BARE_WORDS: &[&str] = &["bare_string", "bare_symbol"];
const STRING: &str = "string";
const STRING_CONTENT: &str = "string_content";
const ESCAPE_SEQUENCE: &str = "escape_sequence";
const INTERPOLATION: &str = "interpolation";
const INTERPOLATION_PLACEHOLDER: &str = "interpolated";
const ESCAPE_PREFIX: char = '\\';
const NEWLINE: &str = "\n";
const TAB: &str = "\t";
const SIMPLE_SYMBOL: &str = "simple_symbol";
const DELIMITED_SYMBOL: &str = "delimited_symbol";
const HASH_KEY_SYMBOL: &str = "hash_key_symbol";
const COMMENT: &str = "comment";
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const TRUE: &str = "true";
const FALSE: &str = "false";
const NIL: &str = "nil";
const LAMBDA: &str = "lambda";
const UNARY: &str = "unary";
const SYMBOL_PREFIX: char = ':';
const DIGIT_SEPARATOR: char = '_';
const SQL_TRUE: &str = "TRUE";
const SQL_FALSE: &str = "FALSE";
const UNKNOWN_VALUE: &str = "''";

#[derive(Debug, Default, Clone)]
pub struct Arguments<'tree> {
    pub positional: Vec<Node<'tree>>,
    pub keywords: HashMap<String, Node<'tree>>,
    pub integer_ids: bool,
}

impl<'tree> Arguments<'tree> {
    pub fn parse(list: Option<Node<'tree>>, source: &[u8]) -> Self {
        let mut arguments = Self::default();
        for argument in list.map(named_children).unwrap_or_default() {
            match argument.kind() {
                PAIR => arguments.add_pair(argument, source),
                HASH => named_children(argument)
                    .into_iter()
                    .filter(|pair| pair.kind() == PAIR)
                    .for_each(|pair| arguments.add_pair(pair, source)),
                COMMENT => {}
                _ => arguments.positional.push(argument),
            }
        }
        arguments
    }

    fn add_pair(&mut self, pair: Node<'tree>, source: &[u8]) {
        let key = pair.child_by_field_name("key").and_then(|key| name(key, source));
        if let (Some(key), Some(value)) = (key, pair.child_by_field_name("value")) {
            self.keywords.insert(key, value);
        }
    }

    pub fn without_first(&self) -> Self {
        Self {
            positional: self.positional.iter().skip(1).copied().collect(),
            keywords: self.keywords.clone(),
            integer_ids: self.integer_ids,
        }
    }

    #[must_use]
    pub const fn with_integer_ids(mut self, integer_ids: bool) -> Self {
        self.integer_ids = integer_ids;
        self
    }

    pub fn keyword(&self, key: &str) -> Option<Node<'tree>> {
        self.keywords.get(key).copied()
    }

    pub fn has(&self, key: &str) -> bool {
        self.keywords.contains_key(key)
    }

    pub fn name_at(&self, index: usize, source: &[u8]) -> Option<String> {
        name(*self.positional.get(index)?, source)
    }

    pub fn keyword_name(&self, key: &str, source: &[u8]) -> Option<String> {
        name(self.keyword(key)?, source)
    }

    pub fn is_true(&self, key: &str) -> bool {
        self.keyword(key).is_some_and(|value| value.kind() == TRUE)
    }

    pub fn is_false(&self, key: &str) -> bool {
        self.keyword(key).is_some_and(|value| value.kind() == FALSE)
    }

    pub fn number(&self, key: &str, source: &[u8]) -> Option<String> {
        self.keyword(key)
            .filter(|value| [INTEGER, FLOAT].contains(&value.kind()))
            .map(|value| raw(value, source).replace(DIGIT_SEPARATOR, ""))
    }
}

pub fn raw(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or_default().to_owned()
}

pub fn name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        SIMPLE_SYMBOL | HASH_KEY_SYMBOL => {
            Some(raw(node, source).trim_start_matches(SYMBOL_PREFIX).to_owned())
        }
        STRING | DELIMITED_SYMBOL => string_content(node, source),
        kind if BARE_WORDS.contains(&kind) => string_content(node, source),
        _ => None,
    }
}

pub fn string_content(node: Node<'_>, source: &[u8]) -> Option<String> {
    let parts = named_children(node);
    if parts.iter().any(|part| part.kind() != STRING_CONTENT) {
        return None;
    }
    Some(parts.iter().map(|part| raw(*part, source)).collect())
}

pub fn executed_sql(node: Node<'_>, source: &[u8]) -> Option<String> {
    let parts = named_children(node);
    if !parts.iter().any(|part| part.kind() == STRING_CONTENT) {
        return None;
    }
    parts
        .iter()
        .map(|part| match part.kind() {
            STRING_CONTENT => Some(raw(*part, source)),
            ESCAPE_SEQUENCE => Some(unescape(&raw(*part, source))),
            INTERPOLATION => Some(INTERPOLATION_PLACEHOLDER.to_owned()),
            _ => None,
        })
        .collect()
}

fn unescape(sequence: &str) -> String {
    match sequence.strip_prefix(ESCAPE_PREFIX) {
        Some("n") => NEWLINE.to_owned(),
        Some("t") => TAB.to_owned(),
        Some(escaped) => escaped.to_owned(),
        None => sequence.to_owned(),
    }
}

pub fn names(node: Node<'_>, source: &[u8]) -> Vec<String> {
    if ARRAY_KINDS.contains(&node.kind()) {
        return named_children(node)
            .into_iter()
            .filter_map(|element| name(element, source))
            .collect();
    }
    name(node, source).into_iter().collect()
}

pub fn is_nil(node: Node<'_>) -> bool {
    node.kind() == NIL
}

pub fn is_empty_collection(node: Node<'_>) -> Option<bool> {
    [HASH, ARRAY]
        .contains(&node.kind())
        .then(|| named_children(node).is_empty())
}

pub fn is_hash(node: Node<'_>) -> bool {
    node.kind() == HASH
}

pub fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

pub fn lambda_body(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() != LAMBDA {
        return None;
    }
    let body = node.child_by_field_name("body")?;
    let block_body = named_children(body).into_iter().next()?;
    let expression = named_children(block_body)
        .into_iter()
        .next()
        .unwrap_or(block_body);
    string_content(expression, source).or_else(|| Some(raw(expression, source)))
}

pub fn sql_literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        NIL => None,
        TRUE => Some(SQL_TRUE.to_owned()),
        FALSE => Some(SQL_FALSE.to_owned()),
        INTEGER | FLOAT | UNARY => Some(raw(node, source).replace(DIGIT_SEPARATOR, "")),
        LAMBDA => lambda_body(node, source),
        _ => Some(name(node, source).map_or_else(|| UNKNOWN_VALUE.to_owned(), |text| quoted(&text))),
    }
}

const HEREDOC_BEGINNING: &str = "heredoc_beginning";
const HEREDOC_BODY: &str = "heredoc_body";
const HEREDOC_END: &str = "heredoc_end";

pub fn heredoc_body(beginning: Node<'_>) -> Option<Node<'_>> {
    if beginning.kind() != HEREDOC_BEGINNING {
        return None;
    }
    let mut ancestor = beginning.parent();
    while let Some(node) = ancestor {
        let body = crate::syntax::descendants(node).into_iter().find(|candidate| {
            candidate.kind() == HEREDOC_BODY && candidate.start_byte() >= beginning.end_byte()
        });
        if body.is_some() {
            return body;
        }
        ancestor = node.parent();
    }
    None
}

pub fn heredoc_text(body: Node<'_>, source: &[u8]) -> String {
    let end = named_children(body)
        .into_iter()
        .find(|part| part.kind() == HEREDOC_END)
        .map_or_else(|| body.end_byte(), |part| part.start_byte());
    source
        .get(body.start_byte()..end)
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .unwrap_or_default()
}

pub fn text(node: Node<'_>, source: &[u8]) -> String {
    if let Some(body) = heredoc_body(node) {
        return heredoc_text(body, source);
    }
    string_content(node, source).unwrap_or_else(|| raw(node, source))
}
