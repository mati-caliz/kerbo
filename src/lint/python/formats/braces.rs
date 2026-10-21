const OPENING: char = '{';
const CLOSING: char = '}';
const SPEC_SEPARATOR: char = ':';
const CONVERSION_SEPARATOR: char = '!';
const ATTRIBUTE: char = '.';
const INDEX_OPENING: char = '[';
const INDEX_CLOSING: char = ']';

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BraceSummary {
    pub autos: Vec<usize>,
    pub indices: Vec<usize>,
    pub keywords: Vec<String>,
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
            summary.record(&nested.name)?;
        }
    }
    Some(summary)
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
    while let Some(first) = rest.chars().next() {
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
