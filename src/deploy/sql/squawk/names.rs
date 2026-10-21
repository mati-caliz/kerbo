use crate::deploy::sql::lexer::{Token, TokenKind};

const UNKNOWN_COLUMN: &str = "?column?";
const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const OPEN_BRACKET: char = '[';
const CLOSE_BRACKET: char = ']';
const QUALIFIER_SEPARATOR: char = '.';
const CAST_MARK: char = ':';
const STAR: char = '*';
const OPERATOR_SYMBOLS: &[char] = &[
    '+', '-', '*', '/', '%', '^', '<', '>', '=', '|', '&', '!', '~', '#', '@', '?',
];
const OPERATOR_WORDS: &[&str] = &[
    "and", "or", "is", "between", "like", "ilike", "similar", "in", "not", "isnull", "notnull",
];
const ARGUMENT_KEYWORDS: &[&str] = &["at", "collate", "zone"];
const NOT_ALIASES: &[&str] = &[
    "end", "null", "true", "false", "as", "from", "else", "then", "and", "or", "not", "is",
];
const SPECIAL_CALLS: &[(&str, &str)] = &[
    ("xmlroot", "xml_root"),
    ("xmlserialize", "xml_serialize"),
    ("xmlelement", "xml_element"),
    ("xmlforest", "xml_forest"),
    ("xmlexists", "xml_exists"),
    ("xmlparse", "xml_parse"),
    ("xmlpi", "xml_pi"),
];
const TYPE_ALIASES: &[(&str, &str)] = &[
    ("bigint", "int8"),
    ("boolean", "bool"),
    ("dec", "numeric"),
    ("decimal", "numeric"),
    ("int", "int4"),
    ("integer", "int4"),
    ("smallint", "int2"),
    ("real", "float4"),
];
const SINGLE_PRECISION_DIGITS: u32 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Name {
    Column(String),
    Unknown(Option<String>),
}

impl Name {
    pub fn text(&self) -> String {
        match self {
            Self::Column(name) | Self::Unknown(Some(name)) => name.clone(),
            Self::Unknown(None) => UNKNOWN_COLUMN.to_owned(),
        }
    }
}

pub fn target_name(tokens: &[Token]) -> Option<(Name, &Token)> {
    if let Some(alias) = explicit_alias(tokens) {
        return Some(alias);
    }
    expression_name(tokens)
}

fn explicit_alias(tokens: &[Token]) -> Option<(Name, &Token)> {
    let (last, before) = tokens.split_last()?;
    let last_name = last.identifier()?;
    let previous = before.last()?;
    if previous.is_word("as") && depth_at(tokens, tokens.len().saturating_sub(1)) == 0 {
        return Some((Name::Column(last_name), last));
    }
    let bare = matches!(last.kind, TokenKind::Quoted(_))
        || last.word().is_some_and(|word| !NOT_ALIASES.contains(&word));
    let completes = previous.is_symbol(CLOSE_PARENTHESIS)
        || previous.is_symbol(CLOSE_BRACKET)
        || previous.kind == TokenKind::Literal
        || (previous.identifier().is_some()
            && !is_operator(previous)
            && !ARGUMENT_KEYWORDS.iter().any(|word| previous.is_word(word)));
    (bare && completes && !is_type_continuation(before)).then_some((Name::Column(last_name), last))
}

fn is_type_continuation(before: &[Token]) -> bool {
    let mut depth = 0_usize;
    for (index, token) in before.iter().enumerate() {
        if token.is_symbol(OPEN_PARENTHESIS) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) {
            depth = depth.saturating_sub(1);
        } else if depth == 0
            && token.is_symbol(CAST_MARK)
            && before
                .get(index.saturating_add(1))
                .is_some_and(|next| next.is_symbol(CAST_MARK))
        {
            return true;
        }
    }
    false
}

pub fn expression_name(tokens: &[Token]) -> Option<(Name, &Token)> {
    let first = tokens.first()?;
    if tokens.last().is_some_and(|last| last.is_symbol(STAR)) {
        return None;
    }
    if let Some(name) = operator_name(tokens) {
        return Some((name, first));
    }
    if let Some(cast) = cast_split(tokens) {
        return cast_name(
            tokens.get(..cast).unwrap_or_default(),
            tokens.get(cast.saturating_add(2)..).unwrap_or_default(),
        );
    }
    if let Some(collate) = top_level_word(tokens, "collate") {
        return expression_name(tokens.get(..collate).unwrap_or_default());
    }
    term_name(tokens)
}

fn operator_name(tokens: &[Token]) -> Option<Name> {
    let at_time_zone = (0..tokens.len()).any(|index| {
        depth_at(tokens, index) == 0
            && tokens.get(index).is_some_and(|token| token.is_word("at"))
            && tokens
                .get(index.saturating_add(1))
                .is_some_and(|next| next.is_word("time") || next.is_word("local"))
    });
    if at_time_zone {
        return Some(Name::Column("timezone".to_owned()));
    }
    if top_level_word(tokens, "overlaps").is_some() {
        return Some(Name::Column("overlaps".to_owned()));
    }
    if top_level_word(tokens, "normalized").is_some() && top_level_word(tokens, "not").is_none() {
        return Some(Name::Column("is_normalized".to_owned()));
    }
    let prefixed = tokens.first().is_some_and(is_operator);
    let infix = (1..tokens.len())
        .any(|index| depth_at(tokens, index) == 0 && tokens.get(index).is_some_and(is_operator));
    (prefixed || infix).then_some(Name::Unknown(None))
}

fn cast_name<'tokens>(inner: &'tokens [Token], typ: &'tokens [Token]) -> Option<(Name, &'tokens Token)> {
    let mut unknown = false;
    if let Some((name, node)) = expression_name(inner) {
        match name {
            Name::Column(_) => return Some((name, node)),
            Name::Unknown(_) => unknown = true,
        }
    }
    let (name, node) = type_name(typ)?;
    Some((
        if unknown {
            Name::Unknown(Some(name))
        } else {
            Name::Column(name)
        },
        node,
    ))
}

fn type_name(typ: &[Token]) -> Option<(String, &Token)> {
    let first = typ.first()?;
    let words: Vec<&str> = typ.iter().filter_map(Token::word).collect();
    let fixed = match words.first().copied().unwrap_or_default() {
        "bit" if words.contains(&"varying") => "varbit",
        "bit" => "bit",
        "varchar" => "varchar",
        "character" | "char" | "nchar" | "national" if words.contains(&"varying") => "varchar",
        "character" | "char" | "nchar" | "national" => "bpchar",
        "double" => "float8",
        "interval" => "interval",
        "time" if words.contains(&"with") => "timetz",
        "time" => "time",
        "timestamp" if words.contains(&"with") => "timestamptz",
        "timestamp" => "timestamp",
        _ => return path_type_name(typ),
    };
    Some((fixed.to_owned(), first))
}

fn path_type_name(typ: &[Token]) -> Option<(String, &Token)> {
    let segment = typ
        .iter()
        .take_while(|token| !token.is_symbol(OPEN_PARENTHESIS) && !token.is_symbol(OPEN_BRACKET))
        .filter(|token| !token.is_symbol(QUALIFIER_SEPARATOR))
        .last()?;
    let name = segment.identifier()?;
    if segment.word().is_some_and(|word| word == "float") {
        let precision = typ.iter().find_map(|token| {
            (token.kind == TokenKind::Literal)
                .then(|| token.raw.parse::<u32>().ok())
                .flatten()
        });
        let float = if precision.is_some_and(|digits| digits <= SINGLE_PRECISION_DIGITS) {
            "float4"
        } else {
            "float8"
        };
        return Some((float.to_owned(), segment));
    }
    let alias = segment
        .word()
        .and_then(|word| TYPE_ALIASES.iter().find(|(from, _)| *from == word))
        .map(|(_, to)| (*to).to_owned());
    Some((alias.unwrap_or(name), segment))
}

fn term_name(tokens: &[Token]) -> Option<(Name, &Token)> {
    let first = tokens.first()?;
    let second = tokens.get(1);
    if first.kind == TokenKind::Literal {
        let national = first.raw.starts_with(['n', 'N']);
        return Some((Name::Unknown(national.then(|| "bpchar".to_owned())), first));
    }
    if let Some(word) = first.word()
        && second.is_some_and(|next| next.kind == TokenKind::Literal)
        && tokens.len() == 2
    {
        return cast_name(
            tokens.get(1..).unwrap_or_default(),
            tokens.get(..1).unwrap_or_default(),
        )
        .or_else(|| Some((Name::Column(word.to_owned()), first)));
    }
    if first.is_symbol(OPEN_PARENTHESIS) {
        return parenthesized_name(tokens);
    }
    if first.is_word("cast") && second.is_some_and(|next| next.is_symbol(OPEN_PARENTHESIS)) {
        let inside = tokens.get(2..tokens.len().saturating_sub(1)).unwrap_or_default();
        let separator = top_level_word(inside, "as")?;
        return cast_name(
            inside.get(..separator).unwrap_or_default(),
            inside.get(separator.saturating_add(1)..).unwrap_or_default(),
        );
    }
    if first.is_word("collation") && second.is_some_and(|next| next.is_word("for")) {
        return Some((Name::Column("pg_collation_for".to_owned()), first));
    }
    if first.is_word("case") {
        return case_name(tokens);
    }
    if (first.is_word("array") || first.is_word("row"))
        && second.is_some_and(|next| next.is_symbol(OPEN_PARENTHESIS) || next.is_symbol(OPEN_BRACKET))
    {
        let name = if first.is_word("array") { "array" } else { "row" };
        return Some((Name::Column(name.to_owned()), first));
    }
    reference_name(tokens)
}

fn parenthesized_name(tokens: &[Token]) -> Option<(Name, &Token)> {
    let close = matching_close(tokens, 0)?;
    let inside = tokens.get(1..close).unwrap_or_default();
    if close.saturating_add(1) < tokens.len() {
        return reference_name(tokens);
    }
    if inside.first().is_some_and(|token| token.is_word("select")) {
        return super::duplicates::select_targets(inside)
            .into_iter()
            .next()
            .and_then(target_name);
    }
    if top_level_comma(inside) {
        return tokens
            .first()
            .map(|first| (Name::Column("row".to_owned()), first));
    }
    expression_name(inside)
}

fn case_name(tokens: &[Token]) -> Option<(Name, &Token)> {
    let first = tokens.first()?;
    let else_index =
        top_level_word(tokens.get(1..).unwrap_or_default(), "else").map(|index| index.saturating_add(1));
    let end_index = tokens.len().saturating_sub(1);
    if let Some(else_index) = else_index
        && let Some((name @ Name::Column(_), node)) = expression_name(
            tokens
                .get(else_index.saturating_add(1)..end_index)
                .unwrap_or_default(),
        )
    {
        return Some((name, node));
    }
    Some((Name::Column("case".to_owned()), first))
}

fn reference_name(tokens: &[Token]) -> Option<(Name, &Token)> {
    let mut segment = None;
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        if token.is_symbol(OPEN_PARENTHESIS) {
            let name = segment?;
            return Some(call_name(tokens, index, name));
        }
        if token.is_symbol(OPEN_BRACKET) {
            index = matching_close(tokens, index)?.saturating_add(1);
            continue;
        }
        if token.identifier().is_some() {
            segment = Some(token);
        } else if !token.is_symbol(QUALIFIER_SEPARATOR) {
            return None;
        }
        index = index.saturating_add(1);
    }
    let segment = segment?;
    Some((Name::Column(segment.identifier()?), segment))
}

fn call_name<'tokens>(tokens: &'tokens [Token], open: usize, name: &'tokens Token) -> (Name, &'tokens Token) {
    let word = name.word().unwrap_or_default();
    let special = match word {
        "trim" => Some(trim_name(
            tokens.get(open.saturating_add(1)..).unwrap_or_default(),
        )),
        _ => SPECIAL_CALLS
            .iter()
            .find(|(from, _)| *from == word)
            .map(|(_, to)| *to),
    };
    let text = special.map_or_else(|| name.identifier().unwrap_or_default(), str::to_owned);
    (Name::Column(text), name)
}

fn trim_name(arguments: &[Token]) -> &'static str {
    match arguments.first().and_then(Token::word) {
        Some("leading") => "ltrim",
        Some("trailing") => "rtrim",
        _ => "btrim",
    }
}

fn is_operator(token: &Token) -> bool {
    match &token.kind {
        TokenKind::Symbol(symbol) => OPERATOR_SYMBOLS.contains(symbol),
        TokenKind::Word(word) => OPERATOR_WORDS.contains(&word.as_str()),
        TokenKind::Quoted(_) | TokenKind::Literal => false,
    }
}

fn depth_at(tokens: &[Token], index: usize) -> usize {
    tokens
        .get(..index)
        .unwrap_or_default()
        .iter()
        .fold(0_usize, |depth, token| {
            if token.is_symbol(OPEN_PARENTHESIS) || token.is_symbol(OPEN_BRACKET) || token.is_word("case") {
                depth.saturating_add(1)
            } else if token.is_symbol(CLOSE_PARENTHESIS)
                || token.is_symbol(CLOSE_BRACKET)
                || token.is_word("end")
            {
                depth.saturating_sub(1)
            } else {
                depth
            }
        })
}

fn top_level_word(tokens: &[Token], word: &str) -> Option<usize> {
    (0..tokens.len()).find(|index| {
        depth_at(tokens, *index) == 0 && tokens.get(*index).is_some_and(|token| token.is_word(word))
    })
}

fn top_level_comma(tokens: &[Token]) -> bool {
    (0..tokens.len()).any(|index| {
        depth_at(tokens, index) == 0 && tokens.get(index).is_some_and(|token| token.is_symbol(','))
    })
}

fn cast_split(tokens: &[Token]) -> Option<usize> {
    (0..tokens.len()).find(|index| {
        depth_at(tokens, *index) == 0
            && tokens.get(*index).is_some_and(|token| token.is_symbol(CAST_MARK))
            && tokens
                .get(index.saturating_add(1))
                .is_some_and(|next| next.is_symbol(CAST_MARK))
    })
}

fn matching_close(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.is_symbol(OPEN_PARENTHESIS) || token.is_symbol(OPEN_BRACKET) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) || token.is_symbol(CLOSE_BRACKET) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}
