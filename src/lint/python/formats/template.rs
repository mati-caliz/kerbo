const OPENING: char = '{';
const CLOSING: char = '}';
const SPEC_SEPARATOR: char = ':';
const CONVERSION_SEPARATOR: char = '!';
const ATTRIBUTE: char = '.';
const INDEX_OPENING: char = '[';
const INDEX_CLOSING: char = ']';
const BACKSLASH: char = '\\';
const NAMED_ESCAPE: &str = "N{";

// Un reemplazo `{…}` del formato: el nombre del campo, la conversión (`!r`) y el especificador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub field_name: String,
    pub conversion: Option<char>,
    pub format_spec: String,
}

// Un pedazo del formato: texto literal (con `{{` y `}}` ya resueltos) o un reemplazo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Literal(String),
    Field(Replacement),
}

// Lo que sigue al argumento en el nombre del campo: `.atributo`, `[0]` o `[clave]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldPart {
    Attribute(String),
    Index(usize),
    StringIndex(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Auto,
    Index(usize),
    Keyword(String),
}

// `FormatString::from_str` (o `from_raw_str`) de `ruff_python_literal`, sobre el texto del string
// tal como está en el fuente, sin las comillas. Devuelve sólo los reemplazos: lo literal no cuenta.
pub fn replacements(contents: &str, raw: bool) -> Option<Vec<Replacement>> {
    Some(
        pieces(contents, raw)?
            .into_iter()
            .filter_map(|piece| match piece {
                Piece::Field(replacement) => Some(replacement),
                Piece::Literal(_) => None,
            })
            .collect(),
    )
}

pub fn pieces(contents: &str, raw: bool) -> Option<Vec<Piece>> {
    let mut found = Vec::new();
    let mut rest = contents;
    while !rest.is_empty() {
        rest = if let Some((value, after)) = literal(rest, raw) {
            found.push(Piece::Literal(value));
            after
        } else {
            let (replacement, after) = bracketed(rest)?;
            found.push(Piece::Field(replacement));
            after
        };
    }
    Some(found)
}

// `parse_literal`: avanza mientras haya texto literal; `None` si no pudo avanzar nada.
fn literal(text: &str, raw: bool) -> Option<(String, &str)> {
    let mut rest = text;
    let mut value = String::new();
    let mut pending_escape = false;
    let mut consumed = false;
    while let Some(first) = rest.chars().next() {
        if pending_escape
            && !raw
            && let Some(after) = named_escape_end(rest)
        {
            value.push_str(rest.get(..rest.len().saturating_sub(after.len()))?);
            rest = after;
            pending_escape = false;
            consumed = true;
            continue;
        }
        if matches!(first, OPENING | CLOSING) {
            if rest.chars().nth(1) != Some(first) {
                return consumed.then_some((value, rest));
            }
            value.push(first);
            rest = rest.get(first.len_utf8().saturating_mul(2)..)?;
            pending_escape = false;
        } else {
            value.push(first);
            rest = rest.get(first.len_utf8()..)?;
            pending_escape = first == BACKSLASH && !pending_escape;
        }
        consumed = true;
    }
    Some((value, rest))
}

// `parse_escaped_unicode_string`: un `\N{NOMBRE}` no abre un reemplazo.
pub fn named_escape_end(text: &str) -> Option<&str> {
    let close = text.strip_prefix(NAMED_ESCAPE)?.find(CLOSING)?;
    text.get(close.saturating_add(NAMED_ESCAPE.len()).saturating_add(1)..)
}

// `parse_spec`: el reemplazo entre llaves, con a lo sumo un nivel de llaves anidadas.
fn bracketed(text: &str) -> Option<(Replacement, &str)> {
    let mut characters = text.char_indices();
    characters.next().filter(|(_, first)| *first == OPENING)?;
    let mut nested = false;
    let mut inside = String::new();
    for (index, character) in characters {
        match character {
            OPENING if nested => return None,
            OPENING => nested = true,
            CLOSING if nested => nested = false,
            CLOSING => return Some((replacement(&inside)?, text.get(index.saturating_add(1)..)?)),
            _ => {}
        }
        inside.push(character);
    }
    None
}

// `parse_part_in_brackets`: el nombre del campo va hasta el primer `:` fuera de corchetes, un
// corchete del nombre sin cerrar es un error y una conversión (`!r`) tiene que ser de un solo
// carácter. Lo que viene después del `:` no se valida.
fn replacement(inside: &str) -> Option<Replacement> {
    let mut argument = String::new();
    let mut characters = inside.chars().peekable();
    let mut format_spec = String::new();
    while let Some(character) = characters.next() {
        if character == SPEC_SEPARATOR {
            format_spec = characters.collect();
            break;
        }
        argument.push(character);
        if character != INDEX_OPENING {
            continue;
        }
        while let Some(next) = characters.next() {
            argument.push(next);
            if next == INDEX_CLOSING {
                break;
            }
            characters.peek()?;
        }
    }
    let (field_name, conversion) = match argument.split_once(CONVERSION_SEPARATOR) {
        Some((name, conversion)) if conversion.chars().count() == 1 => (name, conversion.chars().next()),
        Some(_) => return None,
        None => (argument.as_str(), None),
    };
    Some(Replacement {
        field_name: field_name.to_owned(),
        conversion,
        format_spec,
    })
}

// `FieldName::parse`: el argumento al que apunta el campo, y si después accede a atributos o
// índices. `None` si el nombre está mal formado.
pub fn field_type(field_name: &str) -> Option<FieldType> {
    let split = field_name
        .find([ATTRIBUTE, INDEX_OPENING])
        .unwrap_or(field_name.len());
    let (first, mut rest) = field_name.split_at(split);
    while let Some(marker) = rest.chars().next() {
        let after = rest.get(marker.len_utf8()..)?;
        let (end, skipped) = match marker {
            ATTRIBUTE => {
                let end = after.find([ATTRIBUTE, INDEX_OPENING]).unwrap_or(after.len());
                (end, end)
            }
            INDEX_OPENING => {
                let end = after.find(INDEX_CLOSING)?;
                (end, end.saturating_add(1))
            }
            _ => return None,
        };
        (end > 0).then_some(())?;
        rest = after.get(skipped..)?;
    }
    Some(field_head(first))
}

// Los accesos del nombre del campo, en orden, como el `FieldName::parse` de ruff.
pub fn field_parts(field_name: &str) -> Vec<FieldPart> {
    let split = field_name
        .find([ATTRIBUTE, INDEX_OPENING])
        .unwrap_or(field_name.len());
    let mut rest = field_name.get(split..).unwrap_or_default();
    let mut parts = Vec::new();
    while let Some(marker) = rest.chars().next() {
        let after = rest.get(marker.len_utf8()..).unwrap_or_default();
        if marker == ATTRIBUTE {
            let end = after.find([ATTRIBUTE, INDEX_OPENING]).unwrap_or(after.len());
            parts.push(FieldPart::Attribute(
                after.get(..end).unwrap_or_default().to_owned(),
            ));
            rest = after.get(end..).unwrap_or_default();
        } else {
            let end = after.find(INDEX_CLOSING).unwrap_or(after.len());
            let inside = after.get(..end).unwrap_or_default();
            parts.push(
                inside
                    .parse::<usize>()
                    .map_or_else(|_| FieldPart::StringIndex(inside.to_owned()), FieldPart::Index),
            );
            rest = after.get(end.saturating_add(1)..).unwrap_or_default();
        }
    }
    parts
}

fn field_head(first: &str) -> FieldType {
    if first.is_empty() {
        FieldType::Auto
    } else if let Ok(index) = first.parse::<usize>() {
        FieldType::Index(index)
    } else {
        FieldType::Keyword(first.to_owned())
    }
}
