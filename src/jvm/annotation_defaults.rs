use super::annotations::annotation;
use super::class_file::{Cursor, FOUR_BYTES, Malformed, Pool, descriptor_to_source};

const BOOLEAN: u8 = b'Z';
const INTEGERS: [u8; 3] = *b"ISB";
const STRING: u8 = b's';
const CLASS: u8 = b'c';
const ENUM_CONSTANT: u8 = b'e';
const ANNOTATION: u8 = b'@';
const ARRAY: u8 = b'[';
const VOID_DESCRIPTOR: &str = "V";
const VOID_CLASS: &str = "void.class";
const CLASS_SUFFIX: &str = ".class";
const QUALIFIER_SEPARATOR: char = '.';
const OBJECT_START: char = 'L';
const TRUE: &str = "true";
const FALSE: &str = "false";
const QUOTE: char = '"';
const ESCAPED: [(char, &str); 5] = [
    ('\\', "\\\\"),
    ('"', "\\\""),
    ('\n', "\\n"),
    ('\t', "\\t"),
    ('\r', "\\r"),
];

// El valor por defecto de un elemento de anotación (`AnnotationDefault`), escrito como lo escribe
// el índice para las anotaciones del proyecto: `false`, `0`, `""`, `Void.class`, `{}`. Los enums,
// las anotaciones anidadas, los char, long, float y double quedan sin valor.
pub(super) fn default_value(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<Option<String>, Malformed> {
    let tag = cursor.u8()?;
    match tag {
        BOOLEAN => {
            let value = pool.integer(cursor.u16()?)?;
            Ok(Some(if value == 0 { FALSE } else { TRUE }.to_owned()))
        }
        tag if INTEGERS.contains(&tag) => Ok(Some(pool.integer(cursor.u16()?)?.to_string())),
        STRING => Ok(Some(quoted(pool.text(cursor.u16()?)?))),
        CLASS => Ok(class_literal(pool.text(cursor.u16()?)?)),
        ENUM_CONSTANT => {
            cursor.take(FOUR_BYTES)?;
            Ok(None)
        }
        ANNOTATION => {
            annotation(cursor, pool)?;
            Ok(None)
        }
        ARRAY => {
            let count = cursor.u16()?;
            let elements = (0..count)
                .map(|_| default_value(cursor, pool))
                .collect::<Result<Vec<Option<String>>, Malformed>>()?;
            Ok(elements
                .into_iter()
                .collect::<Option<Vec<String>>>()
                .map(|elements| format!("{{{}}}", elements.join(","))))
        }
        _ => {
            cursor.u16()?;
            Ok(None)
        }
    }
}

fn quoted(value: &str) -> String {
    let mut written = String::from(QUOTE);
    for character in value.chars() {
        match ESCAPED.iter().find(|(raw, _)| *raw == character) {
            Some((_, escape)) => written.push_str(escape),
            None => written.push(character),
        }
    }
    written.push(QUOTE);
    written
}

fn class_literal(descriptor: &str) -> Option<String> {
    if descriptor == VOID_DESCRIPTOR {
        return Some(VOID_CLASS.to_owned());
    }
    if !descriptor.starts_with(OBJECT_START) {
        return None;
    }
    let source = descriptor_to_source(descriptor);
    let simple = source.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(&source);
    Some(format!("{simple}{CLASS_SUFFIX}"))
}
