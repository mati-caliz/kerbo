const PREFIXES: &[char] = &['!', '#'];
const INDIRECTION: char = '!';
const COUNTING: char = '#';
const NAME_EXPANSION_ENDS: &[char] = &['*', '?', '@'];
const SPECIAL_CHARACTERS: &[char] = &['*', '@', '#', '?', '-', '$', '!'];
const ALL_ELEMENTS: char = '@';
const ARRAY_ALL: &str = "[@]";
const ALTERNATIVE: char = '+';
const COLON: char = ':';
const CLOSE_INDEX: char = ']';
const OPEN_INDEX: char = '[';
const DEFAULT_OPERATORS: &[char] = &['-', '=', '?', '+'];
const OFFSET_SEPARATOR: char = ':';

pub const fn is_variable_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

pub fn is_variable_name(name: &str) -> bool {
    name.chars().next().is_some_and(is_variable_start) && name.chars().all(is_variable_character)
}

pub fn reference(braced: &str) -> String {
    let no_prefix = braced.strip_prefix(PREFIXES).unwrap_or(braced);
    name_expansion(braced)
        .or_else(|| take_name(no_prefix))
        .or_else(|| special(no_prefix))
        .or_else(|| special(braced))
        .unwrap_or_else(|| braced.to_owned())
}

fn name_expansion(braced: &str) -> Option<String> {
    let rest = braced.strip_prefix(INDIRECTION)?;
    let next = rest.chars().next()?;
    if !is_variable_character(next) {
        return None;
    }
    let after = rest.get(next.len_utf8()..)?;
    let first = after
        .chars()
        .find(|character| !is_variable_character(*character))?;
    NAME_EXPANSION_ENDS.contains(&first).then(String::new)
}

fn take_name(text: &str) -> Option<String> {
    let name: String = text
        .chars()
        .take_while(|character| is_variable_character(*character))
        .collect();
    (!name.is_empty()).then_some(name)
}

fn special(text: &str) -> Option<String> {
    text.chars()
        .next()
        .filter(|first| SPECIAL_CHARACTERS.contains(first))
        .map(String::from)
}

pub fn modifier(braced: &str) -> String {
    let name = reference(braced);
    let candidates: Vec<&str> = match braced.chars().next() {
        Some(first) if PREFIXES.contains(&first) => {
            vec![braced.get(first.len_utf8()..).unwrap_or_default(), braced]
        }
        _ => vec![braced],
    };
    candidates
        .into_iter()
        .find_map(|candidate| candidate.strip_prefix(name.as_str()))
        .unwrap_or_default()
        .to_owned()
}

pub fn is_array_expansion(braced: &str) -> bool {
    braced.starts_with(ALL_ELEMENTS) || (!braced.starts_with(COUNTING) && braced.contains(ARRAY_ALL))
}

pub fn is_counting(braced: &str) -> bool {
    braced.starts_with(COUNTING)
}

pub fn is_quoted_alternative(braced: &str) -> bool {
    let modifier = modifier(braced);
    let alternative_at = |text: &str| text.strip_prefix(COLON).unwrap_or(text).starts_with(ALTERNATIVE);
    alternative_at(&modifier)
        || modifier
            .match_indices(CLOSE_INDEX)
            .any(|(index, _)| alternative_at(modifier.get(index.saturating_add(1)..).unwrap_or_default()))
}

pub fn index_references(braced: &str) -> Vec<String> {
    let Some(open) = braced.find(OPEN_INDEX) else {
        return Vec::new();
    };
    let Some(close) = braced.rfind(CLOSE_INDEX).filter(|close| *close > open) else {
        return Vec::new();
    };
    variable_names(braced.get(open..=close).unwrap_or_default())
}

pub fn offset_references(modifier: &str) -> Vec<String> {
    let rest = if modifier.starts_with(OPEN_INDEX) {
        match modifier.find(CLOSE_INDEX) {
            Some(close) if close > 1 => modifier.get(close.saturating_add(1)..).unwrap_or_default(),
            _ => return Vec::new(),
        }
    } else {
        modifier
    };
    let Some(offsets) = rest.trim_start_matches(' ').strip_prefix(OFFSET_SEPARATOR) else {
        return Vec::new();
    };
    if offsets.starts_with(DEFAULT_OPERATORS) || offsets.is_empty() {
        return Vec::new();
    }
    variable_names(offsets)
}

pub fn variable_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if !is_variable_start(character) {
            continue;
        }
        let mut name = String::from(character);
        while let Some(next) = characters.next_if(|next| is_variable_character(*next)) {
            name.push(next);
        }
        names.push(name);
    }
    names
}

const fn is_variable_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}
