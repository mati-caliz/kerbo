use super::template::named_escape_end;

const BACKSLASH: char = '\\';
const OPENING: char = '{';
const CLOSING: char = '}';
const SPEC_SEPARATOR: char = ':';
const CONVERSION_SEPARATOR: char = '!';
const ATTRIBUTE: char = '.';
const INDEX_OPENING: char = '[';
const INDEX_CLOSING: char = ']';
const ALIGNMENTS: [char; 4] = ['<', '>', '=', '^'];
const SIGNS: [char; 3] = ['+', '-', ' '];
const ALTERNATE_FORM: char = '#';
const ZERO: char = '0';
const GROUPINGS: [char; 2] = [',', '_'];
const PRECISION: char = '.';
const CONVERSIONS: [char; 4] = ['s', 'r', 'a', 'b'];
const FORMAT_TYPES: [char; 16] = [
    's', 'b', 'c', 'd', 'o', 'n', 'N', 'x', 'X', 'e', 'E', 'f', 'F', 'g', 'G', '%',
];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BraceSummary {
    pub autos: Vec<usize>,
    pub indices: Vec<usize>,
    pub keywords: Vec<String>,
    pub has_nested_parts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    name: String,
    spec: String,
}

pub fn summary(format: &str) -> Option<BraceSummary> {
    let mut summary = BraceSummary::default();
    for field in fields(format)? {
        summary.record(&field.name)?;
        for nested in fields(&field.spec)? {
            summary.has_nested_parts = true;
            summary.record(&nested.name)?;
        }
    }
    Some(summary)
}

// PLE1300: los tipos de formato que no existen (`"{:z}"`), con el `FormatSpec::parse` de ruff. Un
// especificador con llaves adentro sólo se revisa en las llaves de adentro.
pub fn invalid_format_types(format: &str) -> Vec<char> {
    let Some(found) = fields(format) else {
        return Vec::new();
    };
    found
        .iter()
        .flat_map(|field| {
            if field.spec.contains(OPENING) {
                fields(&field.spec)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|nested| invalid_format_type(&nested.spec))
                    .collect()
            } else {
                invalid_format_type(&field.spec)
                    .into_iter()
                    .collect::<Vec<char>>()
            }
        })
        .collect()
}

fn invalid_format_type(spec: &str) -> Option<char> {
    let mut rest = spec;
    let mut characters = rest.chars();
    if characters.next() == Some(CONVERSION_SEPARATOR)
        && characters
            .next()
            .is_some_and(|conversion| CONVERSIONS.contains(&conversion))
    {
        rest = characters.as_str();
    }
    rest = without_fill_and_align(rest);
    rest = rest.strip_prefix(SIGNS).unwrap_or(rest);
    rest = rest.strip_prefix(ALTERNATE_FORM).unwrap_or(rest);
    rest = rest.strip_prefix(ZERO).unwrap_or(rest);
    rest = without_digits(rest)?;
    rest = rest.strip_prefix(GROUPINGS).unwrap_or(rest);
    if let Some(precision) = rest.strip_prefix(PRECISION) {
        let after = without_digits(precision)?;
        if after.len() < precision.len() {
            rest = after;
        }
    }
    rest.chars()
        .next()
        .filter(|format_type| !FORMAT_TYPES.contains(format_type))
}

fn without_fill_and_align(spec: &str) -> &str {
    let mut characters = spec.chars();
    let Some(first) = characters.next() else {
        return spec;
    };
    let after_fill = characters.as_str();
    match after_fill.strip_prefix(ALIGNMENTS) {
        Some(rest) => rest,
        None if ALIGNMENTS.contains(&first) => after_fill,
        None => spec,
    }
}

fn without_digits(text: &str) -> Option<&str> {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    let rest = text.get(digits..)?;
    (digits == 0 || text.get(..digits)?.parse::<usize>().is_ok()).then_some(rest)
}

impl BraceSummary {
    fn record(&mut self, name: &str) -> Option<()> {
        let first = field_head(name)?;
        if first.is_empty() {
            self.autos.push(self.autos.len());
        } else if let Ok(index) = first.parse::<usize>() {
            self.indices.push(index);
        } else {
            self.keywords.push(first.to_owned());
        }
        Some(())
    }
}

fn fields(format: &str) -> Option<Vec<Field>> {
    let mut found = Vec::new();
    let mut rest = format;
    let mut pending_escape = false;
    while let Some(first) = rest.chars().next() {
        if pending_escape && let Some(after) = named_escape_end(rest) {
            rest = after;
            pending_escape = false;
            continue;
        }
        pending_escape = first == BACKSLASH && !pending_escape;
        let doubled = rest.chars().nth(1) == Some(first);
        if first == OPENING && !doubled {
            let (field, after) = bracketed(rest)?;
            found.push(field);
            rest = after;
        } else if first == CLOSING && !doubled {
            return None;
        } else {
            let skipped = if matches!(first, OPENING | CLOSING) {
                2
            } else {
                first.len_utf8()
            };
            rest = rest.get(skipped..)?;
        }
    }
    Some(found)
}

fn bracketed(text: &str) -> Option<(Field, &str)> {
    let mut nested = false;
    let mut inside = String::new();
    for (index, character) in text.char_indices().skip(1) {
        match character {
            OPENING if nested => return None,
            OPENING => nested = true,
            CLOSING if nested => nested = false,
            CLOSING => return Some((field(&inside)?, text.get(index.saturating_add(1)..)?)),
            _ => {}
        }
        inside.push(character);
    }
    None
}

fn field(inside: &str) -> Option<Field> {
    let (argument, spec) = inside.split_once(SPEC_SEPARATOR).unwrap_or((inside, ""));
    let name = match argument.split_once(CONVERSION_SEPARATOR) {
        Some((name, conversion)) if conversion.chars().count() == 1 => name,
        Some(_) => return None,
        None => argument,
    };
    Some(Field {
        name: name.to_owned(),
        spec: spec.to_owned(),
    })
}

fn field_head(name: &str) -> Option<&str> {
    let split = name.find([ATTRIBUTE, INDEX_OPENING]).unwrap_or(name.len());
    let (head, mut rest) = name.split_at(split);
    while let Some(marker) = rest.chars().next() {
        let after = rest.get(marker.len_utf8()..)?;
        rest = if marker == ATTRIBUTE {
            let end = after.find([ATTRIBUTE, INDEX_OPENING]).unwrap_or(after.len());
            (end > 0).then_some(())?;
            after.get(end..)?
        } else if marker == INDEX_OPENING {
            let end = after.find(INDEX_CLOSING)?;
            (end > 0).then_some(())?;
            after.get(end.saturating_add(1)..)?
        } else {
            return None;
        };
    }
    Some(head)
}
