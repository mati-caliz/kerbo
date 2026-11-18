use super::class_file::{Cursor, Malformed};
use super::constant_pool::{
    CLASS, DOUBLE, DYNAMIC, FIELD_REFERENCE, INTERFACE_METHOD_REFERENCE, INVOKE_DYNAMIC, LONG,
    METHOD_REFERENCE, NAME_AND_TYPE, UTF8, constant_size,
};

#[derive(Debug)]
enum Constant {
    Text(String),
    Class(u16),
    NameAndType { name: u16, descriptor: u16 },
    Member { class: u16, name_and_type: u16 },
    Dynamic(u16),
    Other,
}

// El pool de constantes para leer bytecode: además de los textos y las clases, a qué campo o método
// apunta cada instrucción.
#[derive(Debug)]
pub(super) struct CodePool {
    constants: Vec<Constant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MemberReference<'pool> {
    pub(super) owner: &'pool str,
    pub(super) name: &'pool str,
    pub(super) descriptor: &'pool str,
}

impl CodePool {
    pub(super) fn text(&self, index: u16) -> Option<&str> {
        match self.constants.get(usize::from(index))? {
            Constant::Text(text) => Some(text),
            Constant::Class(_)
            | Constant::NameAndType { .. }
            | Constant::Member { .. }
            | Constant::Dynamic(_)
            | Constant::Other => None,
        }
    }

    pub(super) fn class(&self, index: u16) -> Option<&str> {
        match self.constants.get(usize::from(index))? {
            Constant::Class(name) => self.text(*name),
            Constant::Text(_)
            | Constant::NameAndType { .. }
            | Constant::Member { .. }
            | Constant::Dynamic(_)
            | Constant::Other => None,
        }
    }

    pub(super) fn member(&self, index: u16) -> Option<MemberReference<'_>> {
        let Constant::Member { class, name_and_type } = self.constants.get(usize::from(index))? else {
            return None;
        };
        let (name, descriptor) = self.name_and_type(*name_and_type)?;
        Some(MemberReference {
            owner: self.class(*class)?,
            name,
            descriptor,
        })
    }

    pub(super) fn dynamic_descriptor(&self, index: u16) -> Option<&str> {
        let Constant::Dynamic(name_and_type) = self.constants.get(usize::from(index))? else {
            return None;
        };
        self.name_and_type(*name_and_type)
            .map(|(_, descriptor)| descriptor)
    }

    fn name_and_type(&self, index: u16) -> Option<(&str, &str)> {
        let Constant::NameAndType { name, descriptor } = self.constants.get(usize::from(index))? else {
            return None;
        };
        Some((self.text(*name)?, self.text(*descriptor)?))
    }
}

pub(super) fn code_pool(cursor: &mut Cursor<'_>) -> Result<CodePool, Malformed> {
    let count = usize::from(cursor.u16()?);
    let mut constants = vec![Constant::Other];
    while constants.len() < count {
        let tag = cursor.u8()?;
        constants.push(constant(cursor, tag)?);
        if tag == LONG || tag == DOUBLE {
            constants.push(Constant::Other);
        }
    }
    Ok(CodePool { constants })
}

fn constant(cursor: &mut Cursor<'_>, tag: u8) -> Result<Constant, Malformed> {
    Ok(match tag {
        UTF8 => {
            let length = usize::from(cursor.u16()?);
            Constant::Text(String::from_utf8_lossy(cursor.take(length)?).into_owned())
        }
        CLASS => Constant::Class(cursor.u16()?),
        NAME_AND_TYPE => Constant::NameAndType {
            name: cursor.u16()?,
            descriptor: cursor.u16()?,
        },
        FIELD_REFERENCE | METHOD_REFERENCE | INTERFACE_METHOD_REFERENCE => Constant::Member {
            class: cursor.u16()?,
            name_and_type: cursor.u16()?,
        },
        DYNAMIC | INVOKE_DYNAMIC => {
            cursor.u16()?;
            Constant::Dynamic(cursor.u16()?)
        }
        _ => {
            cursor.take(constant_size(tag).ok_or(Malformed)?)?;
            Constant::Other
        }
    })
}
