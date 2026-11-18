use super::class_file::{Cursor, Malformed};
use super::code_pool::{CodePool, code_pool};

const MAGIC: u32 = 0xCAFE_BABE;
const VERSION_SIZE: usize = 4;
const INTERFACE_SIZE: usize = 2;
const FIELD_HEADER_SIZE: usize = 6;
const CODE: &str = "Code";
const CONSTRUCTOR: &str = "<init>";
const ACCESS_PRIVATE: u16 = 0x0002;
const ACCESS_STATIC: u16 = 0x0008;
const ACCESS_FINAL: u16 = 0x0010;
const WIDE_DESCRIPTORS: [&str; 2] = ["J", "D"];
const VOID_DESCRIPTOR: &str = "V";
const OBJECT_START: char = 'L';
const OBJECT_END: char = ';';
const ARRAY: char = '[';
const ARGUMENTS_OPEN: char = '(';
const ARGUMENTS_CLOSE: char = ')';
const WIDE: usize = 2;
const NARROW: usize = 1;

// El bytecode de una clase: por método, las instrucciones sin decodificar y sus bloques `try`.
#[derive(Debug)]
pub struct ClassCode {
    pub(super) name: String,
    pub(super) pool: CodePool,
    pub(super) is_final: bool,
    pub(super) methods: Vec<MethodCode>,
}

#[derive(Debug)]
pub(super) struct MethodCode {
    pub(super) name: String,
    pub(super) descriptor: String,
    pub(super) access: u16,
    pub(super) code: Option<RawCode>,
}

#[derive(Debug)]
pub(super) struct RawCode {
    pub(super) max_locals: usize,
    pub(super) bytes: Vec<u8>,
    pub(super) handlers: Vec<RawHandler>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RawHandler {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) target: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ArgumentType {
    pub(super) width: usize,
    pub(super) is_reference: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MethodShape {
    pub(super) arguments: Vec<ArgumentType>,
    pub(super) returned: usize,
}

impl ClassCode {
    pub(super) fn method(&self, name: &str, descriptor: &str) -> Option<&MethodCode> {
        self.methods
            .iter()
            .find(|method| method.name == name && method.descriptor == descriptor)
    }

    // Lo que IntelliJ llama un método estable: nadie lo puede redefinir.
    pub(super) fn is_stable(&self, method: &MethodCode) -> bool {
        self.is_final
            || method.access & (ACCESS_FINAL | ACCESS_STATIC | ACCESS_PRIVATE) != 0
            || method.name == CONSTRUCTOR
    }
}

impl MethodCode {
    pub(super) const fn is_static(&self) -> bool {
        self.access & ACCESS_STATIC != 0
    }
}

pub(super) fn parse_code(bytes: &[u8]) -> Result<ClassCode, Malformed> {
    let mut cursor = Cursor { bytes, offset: 0 };
    if cursor.u32()? != MAGIC {
        return Err(Malformed);
    }
    cursor.take(VERSION_SIZE)?;
    let pool = code_pool(&mut cursor)?;
    let access = cursor.u16()?;
    let name = pool.class(cursor.u16()?).ok_or(Malformed)?.to_owned();
    cursor.u16()?;
    let interfaces = usize::from(cursor.u16()?);
    cursor.take(interfaces * INTERFACE_SIZE)?;
    let fields = cursor.u16()?;
    for _ in 0..fields {
        cursor.take(FIELD_HEADER_SIZE)?;
        skip_attributes(&mut cursor)?;
    }
    let method_count = cursor.u16()?;
    let methods = (0..method_count)
        .map(|_| method(&mut cursor, &pool))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ClassCode {
        name,
        pool,
        is_final: access & ACCESS_FINAL != 0,
        methods,
    })
}

fn skip_attributes(cursor: &mut Cursor<'_>) -> Result<(), Malformed> {
    let count = cursor.u16()?;
    for _ in 0..count {
        cursor.u16()?;
        let length = cursor.length()?;
        cursor.take(length)?;
    }
    Ok(())
}

fn method(cursor: &mut Cursor<'_>, pool: &CodePool) -> Result<MethodCode, Malformed> {
    let access = cursor.u16()?;
    let name = pool.text(cursor.u16()?).ok_or(Malformed)?.to_owned();
    let descriptor = pool.text(cursor.u16()?).ok_or(Malformed)?.to_owned();
    let mut code = None;
    let attribute_count = cursor.u16()?;
    for _ in 0..attribute_count {
        let attribute = pool.text(cursor.u16()?).ok_or(Malformed)?;
        let length = cursor.length()?;
        let mut body = Cursor {
            bytes: cursor.take(length)?,
            offset: 0,
        };
        if attribute == CODE {
            code = Some(raw_code(&mut body)?);
        }
    }
    Ok(MethodCode {
        name,
        descriptor,
        access,
        code,
    })
}

fn raw_code(body: &mut Cursor<'_>) -> Result<RawCode, Malformed> {
    body.u16()?;
    let max_locals = usize::from(body.u16()?);
    let length = body.length()?;
    let bytes = body.take(length)?.to_vec();
    let handler_count = body.u16()?;
    let handlers = (0..handler_count)
        .map(|_| {
            let start = usize::from(body.u16()?);
            let end = usize::from(body.u16()?);
            let target = usize::from(body.u16()?);
            body.u16()?;
            Ok(RawHandler { start, end, target })
        })
        .collect::<Result<Vec<_>, Malformed>>()?;
    Ok(RawCode {
        max_locals,
        bytes,
        handlers,
    })
}

// Cuántos lugares de la pila ocupa cada argumento de `(ILjava/lang/String;[J)V` y lo que devuelve.
pub(super) fn method_shape(descriptor: &str) -> Option<MethodShape> {
    let (written, returned) = descriptor
        .strip_prefix(ARGUMENTS_OPEN)?
        .split_once(ARGUMENTS_CLOSE)?;
    let mut arguments = Vec::new();
    let mut rest = written;
    while !rest.is_empty() {
        let (argument, after) = next_argument(rest)?;
        arguments.push(argument);
        rest = after;
    }
    let returned = if returned == VOID_DESCRIPTOR {
        0
    } else {
        type_width(returned)
    };
    Some(MethodShape { arguments, returned })
}

fn next_argument(written: &str) -> Option<(ArgumentType, &str)> {
    let element = written.trim_start_matches(ARRAY);
    let is_array = element.len() != written.len();
    let length = if element.starts_with(OBJECT_START) {
        element.find(OBJECT_END)? + OBJECT_END.len_utf8()
    } else {
        element.chars().next()?.len_utf8()
    };
    let (argument, after) = element.split_at_checked(length)?;
    let is_reference = is_array || argument.starts_with(OBJECT_START);
    let width = if is_reference {
        NARROW
    } else {
        type_width(argument)
    };
    Some((ArgumentType { width, is_reference }, after))
}

pub(super) fn type_width(descriptor: &str) -> usize {
    if WIDE_DESCRIPTORS.contains(&descriptor) {
        WIDE
    } else {
        NARROW
    }
}
