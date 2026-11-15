use super::class_file::{Cursor, FOUR_BYTES, Malformed, THREE_BYTES, TWO_BYTES, binary_to_source};

const UTF8: u8 = 1;
const INTEGER: u8 = 3;
const FLOAT: u8 = 4;
const LONG: u8 = 5;
const DOUBLE: u8 = 6;
const CLASS: u8 = 7;
const STRING: u8 = 8;
const FIELD_REFERENCE: u8 = 9;
const METHOD_REFERENCE: u8 = 10;
const INTERFACE_METHOD_REFERENCE: u8 = 11;
const NAME_AND_TYPE: u8 = 12;
const METHOD_HANDLE: u8 = 15;
const METHOD_TYPE: u8 = 16;
const DYNAMIC: u8 = 17;
const INVOKE_DYNAMIC: u8 = 18;
const MODULE: u8 = 19;
const PACKAGE: u8 = 20;
const EIGHT_BYTES: usize = 8;

// El pool de constantes: hacen falta los textos, las clases (que apuntan a un texto) y los enteros
// (los valores por defecto de las anotaciones).
pub(super) struct Pool {
    texts: Vec<Option<String>>,
    classes: Vec<Option<u16>>,
    integers: Vec<Option<i64>>,
}

impl Pool {
    pub(super) fn text(&self, index: u16) -> Result<&str, Malformed> {
        self.texts
            .get(usize::from(index))
            .and_then(Option::as_deref)
            .ok_or(Malformed)
    }

    pub(super) fn integer(&self, index: u16) -> Result<i64, Malformed> {
        self.integers
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(Malformed)
    }

    pub(super) fn class(&self, index: u16) -> Result<String, Malformed> {
        let name = self
            .classes
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(Malformed)?;
        Ok(binary_to_source(self.text(name)?))
    }
}

pub(super) fn constant_pool(cursor: &mut Cursor<'_>) -> Result<Pool, Malformed> {
    let count = usize::from(cursor.u16()?);
    let mut pool = Pool {
        texts: vec![None; count],
        classes: vec![None; count],
        integers: vec![None; count],
    };
    let mut index = 1;
    while index < count {
        let tag = cursor.u8()?;
        let mut slots = 1;
        match tag {
            UTF8 => {
                let length = usize::from(cursor.u16()?);
                let text = String::from_utf8_lossy(cursor.take(length)?).into_owned();
                if let Some(slot) = pool.texts.get_mut(index) {
                    *slot = Some(text);
                }
            }
            CLASS => {
                let name = cursor.u16()?;
                if let Some(slot) = pool.classes.get_mut(index) {
                    *slot = Some(name);
                }
            }
            STRING | METHOD_TYPE | MODULE | PACKAGE => {
                cursor.take(TWO_BYTES)?;
            }
            METHOD_HANDLE => {
                cursor.take(THREE_BYTES)?;
            }
            INTEGER => {
                let value = i32::from_be_bytes(cursor.u32()?.to_be_bytes());
                if let Some(slot) = pool.integers.get_mut(index) {
                    *slot = Some(i64::from(value));
                }
            }
            FLOAT
            | FIELD_REFERENCE
            | METHOD_REFERENCE
            | INTERFACE_METHOD_REFERENCE
            | NAME_AND_TYPE
            | DYNAMIC
            | INVOKE_DYNAMIC => {
                cursor.take(FOUR_BYTES)?;
            }
            LONG | DOUBLE => {
                cursor.take(EIGHT_BYTES)?;
                slots = 2;
            }
            _ => return Err(Malformed),
        }
        index = index.saturating_add(slots);
    }
    Ok(pool)
}
