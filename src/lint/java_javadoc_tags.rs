const JAVADOC_OPENER: &str = "/**";
const COMMENT_CLOSER: &str = "*/";
const LEADING_STAR: char = '*';
const TAG_MARK: char = '@';
const INHERIT_DOC: &str = "inheritDoc";
const INLINE_OPENER: char = '{';
const INLINE_CLOSER: char = '}';
const CARRIAGE_RETURN: char = '\r';
const LINE_FEED: char = '\n';
const PARAM: &str = "param";
const RETURN: &str = "return";
const SEE: &str = "see";
const THROWS: [&str; 2] = ["throws", "exception"];
const KNOWN_BLOCK_TAGS: [&str; 15] = [
    "author",
    "deprecated",
    "exception",
    "hidden",
    PARAM,
    "provides",
    RETURN,
    SEE,
    "serial",
    "serialData",
    "serialField",
    "since",
    "throws",
    "uses",
    "version",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodTagKind {
    Param,
    Throws,
    Return,
    See,
    InheritDoc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodTag {
    pub kind: MethodTagKind,
    pub argument: String,
    pub row_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTag {
    pub name: String,
    pub argument: String,
    pub row_offset: usize,
}

impl BlockTag {
    pub fn is_known(&self) -> bool {
        KNOWN_BLOCK_TAGS.contains(&self.name.as_str())
    }

    pub fn is_param(&self) -> bool {
        self.name == PARAM
    }
}

// El `\s` de las expresiones de Java: sin los espacios de Unicode.
const fn is_java_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')
}

fn trim_java_start(text: &str) -> &str {
    text.trim_start_matches(is_java_space)
}

fn comment_lines(comment: &str) -> Vec<&str> {
    comment
        .split(LINE_FEED)
        .map(|line| line.strip_suffix(CARRIAGE_RETURN).unwrap_or(line))
        .collect()
}

fn first_word(text: &str) -> &str {
    text.split(is_java_space).next().unwrap_or_default()
}

// `^\s*(?>\*|\/\*\*)?\s*@`: lo que sigue a la arroba si la línea arranca con una etiqueta.
fn after_tag_mark(line: &str) -> Option<&str> {
    let trimmed = trim_java_start(line);
    [
        trimmed.strip_prefix(LEADING_STAR),
        trimmed.strip_prefix(JAVADOC_OPENER),
        Some(trimmed),
    ]
    .into_iter()
    .flatten()
    .find_map(|rest| trim_java_start(rest).strip_prefix(TAG_MARK))
}

fn tag_rest<'line>(after_mark: &'line str, names: &[&'static str]) -> Option<(&'static str, &'line str)> {
    names
        .iter()
        .find_map(|name| after_mark.strip_prefix(name).map(|rest| (*name, rest)))
}

fn argument_tag(line: &str) -> Option<(MethodTagKind, String)> {
    let (name, rest) = tag_rest(after_tag_mark(line)?, &[THROWS[0], THROWS[1], PARAM])?;
    if !rest.starts_with(is_java_space) {
        return None;
    }
    let argument = first_word(trim_java_start(rest));
    if argument.is_empty() {
        return None;
    }
    let kind = if name == PARAM {
        MethodTagKind::Param
    } else {
        MethodTagKind::Throws
    };
    Some((kind, argument.to_owned()))
}

fn plain_kind(name: &str) -> MethodTagKind {
    if name == RETURN {
        MethodTagKind::Return
    } else {
        MethodTagKind::See
    }
}

// `@return` o `@see` con algo escrito en la misma línea.
fn plain_tag(line: &str) -> Option<MethodTagKind> {
    let (name, rest) = tag_rest(after_tag_mark(line)?, &[RETURN, SEE])?;
    let described = rest.starts_with(is_java_space) && !trim_java_start(rest).is_empty();
    described.then(|| plain_kind(name))
}

// `@return` o `@see` solos al final de la línea: valen si lo próximo escrito no es otra etiqueta ni
// el cierre del comentario.
fn multiline_start(line: &str) -> Option<MethodTagKind> {
    let (name, rest) = tag_rest(after_tag_mark(line)?, &[RETURN, SEE])?;
    trim_java_start(rest).is_empty().then(|| plain_kind(name))
}

fn continues_with_text(line: &str) -> Option<bool> {
    line.char_indices().find_map(|(index, character)| {
        if line
            .get(index..)
            .is_some_and(|rest| rest.starts_with(COMMENT_CLOSER))
            || character == TAG_MARK
        {
            Some(false)
        } else if !is_java_space(character) && character != LEADING_STAR {
            Some(true)
        } else {
            None
        }
    })
}

fn has_inherit_doc(line: &str) -> bool {
    line.match_indices(INLINE_OPENER).any(|(index, _)| {
        line.get(index.saturating_add(1)..)
            .map(trim_java_start)
            .and_then(|rest| rest.strip_prefix(TAG_MARK))
            .and_then(|rest| rest.strip_prefix(INHERIT_DOC))
            .is_some_and(|rest| trim_java_start(rest).starts_with(INLINE_CLOSER))
    })
}

// Las etiquetas que mira JavadocMethod, con las expresiones de `getMethodTags` de Checkstyle.
pub fn method_tags(comment: &str) -> Vec<MethodTag> {
    let lines = comment_lines(comment);
    let mut tags = Vec::new();
    for (row_offset, line) in lines.iter().enumerate() {
        let found = if let Some((kind, argument)) = argument_tag(line) {
            Some((kind, argument))
        } else if let Some(kind) = plain_tag(line) {
            Some((kind, String::new()))
        } else if has_inherit_doc(line) {
            Some((MethodTagKind::InheritDoc, String::new()))
        } else {
            multiline_start(line)
                .filter(|_| {
                    lines
                        .iter()
                        .skip(row_offset.saturating_add(1))
                        .find_map(|next| continues_with_text(next))
                        .unwrap_or(false)
                })
                .map(|kind| (kind, String::new()))
        };
        if let Some((kind, argument)) = found {
            tags.push(MethodTag {
                kind,
                argument,
                row_offset,
            });
        }
    }
    tags
}

fn block_tag_start(line: &str, is_first: bool) -> Option<&str> {
    let line = if is_first {
        line.strip_prefix(JAVADOC_OPENER).unwrap_or(line)
    } else {
        line
    };
    let rest = trim_java_start(line).trim_start_matches(LEADING_STAR);
    trim_java_start(rest).strip_prefix(TAG_MARK)
}

// Cuántas llaves de tags inline (`{@snippet : ...}`, `{@code ...}`) quedan abiertas al terminar la
// línea: adentro de uno, las llaves del código de ejemplo también cuentan.
fn inline_depth_after(line: &str, depth_before: usize) -> usize {
    let mut depth = depth_before;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        if character == INLINE_OPENER && (depth > 0 || characters.peek() == Some(&TAG_MARK)) {
            depth = depth.saturating_add(1);
        } else if character == INLINE_CLOSER {
            depth = depth.saturating_sub(1);
        }
    }
    depth
}

// Las etiquetas de bloque del árbol de Javadoc de Checkstyle: una arroba al principio de la línea,
// después de los blancos y los asteriscos, fuera de un tag inline.
pub fn block_tags(comment: &str) -> Vec<BlockTag> {
    let lines = comment_lines(comment);
    let mut depth = 0;
    let outside_inline: Vec<bool> = lines
        .iter()
        .map(|line| {
            let outside = depth == 0;
            depth = inline_depth_after(line, depth);
            outside
        })
        .collect();
    lines
        .into_iter()
        .enumerate()
        .filter(|(row_offset, _)| outside_inline.get(*row_offset).copied().unwrap_or_default())
        .filter_map(|(row_offset, line)| {
            let after_mark = block_tag_start(line, row_offset == 0)?;
            let name = first_word(after_mark).trim_end_matches(COMMENT_CLOSER);
            if name.is_empty() {
                return None;
            }
            let rest = after_mark.get(name.len()..).unwrap_or_default();
            let argument = first_word(trim_java_start(rest)).trim_end_matches(COMMENT_CLOSER);
            Some(BlockTag {
                name: name.to_owned(),
                argument: argument.to_owned(),
                row_offset,
            })
        })
        .collect()
}
