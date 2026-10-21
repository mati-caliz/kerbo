use super::class_file::{Cursor, FOUR_BYTES, Malformed, Pool, THREE_BYTES, TWO_BYTES, descriptor_to_source};

const ENUM_CONSTANT_VALUE: u8 = b'e';
const ANNOTATION_VALUE: u8 = b'@';
const ARRAY_VALUE: u8 = b'[';
// Los tipos de destino de una anotación de tipo (JVMS 4.7.20.1), agrupados por cuánto ocupa lo que
// sigue: el del parámetro formal es el único que interesa.
const FORMAL_PARAMETER_TARGET: u8 = 0x16;
const TYPE_PARAMETER_TARGETS: [u8; 2] = [0x00, 0x01];
const TWO_BYTE_TARGETS: [u8; 8] = [0x10, 0x17, 0x42, 0x43, 0x44, 0x45, 0x46, 0x11];
const BOUND_TARGETS: [u8; 1] = [0x12];
const EMPTY_TARGETS: [u8; 3] = [0x13, 0x14, 0x15];
const LOCAL_VARIABLE_TARGETS: [u8; 2] = [0x40, 0x41];
const TYPE_ARGUMENT_TARGETS: [u8; 5] = [0x47, 0x48, 0x49, 0x4A, 0x4B];
const LOCAL_VARIABLE_ENTRY: usize = 6;

pub(super) fn annotation_list(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<Vec<String>, Malformed> {
    let count = cursor.u16()?;
    (0..count).map(|_| annotation(cursor, pool)).collect()
}

// El tipo de una anotación (de `Lorg/jspecify/annotations/Nullable;` a su nombre completo),
// salteando sus valores.
fn annotation(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<String, Malformed> {
    let name = descriptor_to_source(pool.text(cursor.u16()?)?);
    let pairs = cursor.u16()?;
    for _ in 0..pairs {
        cursor.u16()?;
        skip_element_value(cursor, pool)?;
    }
    Ok(name)
}

fn skip_element_value(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<(), Malformed> {
    match cursor.u8()? {
        ENUM_CONSTANT_VALUE => {
            cursor.take(FOUR_BYTES)?;
        }
        ANNOTATION_VALUE => {
            annotation(cursor, pool)?;
        }
        ARRAY_VALUE => {
            let count = cursor.u16()?;
            for _ in 0..count {
                skip_element_value(cursor, pool)?;
            }
        }
        _ => {
            cursor.u16()?;
        }
    }
    Ok(())
}

// Las anotaciones de tipo sobre los parámetros formales (target 0x16), con el índice del
// parámetro. Las demás se leen sólo para saltearlas.
pub(super) fn type_annotations(
    cursor: &mut Cursor<'_>,
    pool: &Pool,
) -> Result<Vec<(usize, String)>, Malformed> {
    let count = cursor.u16()?;
    let mut found = Vec::new();
    for _ in 0..count {
        let target = cursor.u8()?;
        let parameter = skip_target_info(cursor, target)?;
        let path_length = usize::from(cursor.u8()?);
        cursor.take(path_length.saturating_mul(TWO_BYTES))?;
        let name = annotation(cursor, pool)?;
        if let Some(parameter) = parameter.filter(|_| path_length == 0) {
            found.push((parameter, name));
        }
    }
    Ok(found)
}

fn skip_target_info(cursor: &mut Cursor<'_>, target: u8) -> Result<Option<usize>, Malformed> {
    if target == FORMAL_PARAMETER_TARGET {
        return Ok(Some(usize::from(cursor.u8()?)));
    }
    if TYPE_PARAMETER_TARGETS.contains(&target) {
        cursor.u8()?;
    } else if TWO_BYTE_TARGETS.contains(&target) || BOUND_TARGETS.contains(&target) {
        cursor.u16()?;
    } else if LOCAL_VARIABLE_TARGETS.contains(&target) {
        let entries = usize::from(cursor.u16()?);
        cursor.take(entries.saturating_mul(LOCAL_VARIABLE_ENTRY))?;
    } else if TYPE_ARGUMENT_TARGETS.contains(&target) {
        cursor.take(THREE_BYTES)?;
    } else if !EMPTY_TARGETS.contains(&target) {
        return Err(Malformed);
    }
    Ok(None)
}
