// Lo que kerbo necesita de un `.class`, sin bytecode: el nombre, los padres, las anotaciones de la
// clase y, por método, el nombre, el descriptor, la visibilidad y las anotaciones de cada
// parámetro (las de declaración y las de tipo, como el `@Nullable` de JSpecify).

use super::annotations::{TypeTarget, annotation_list, type_annotations};
use super::method::{ARRAY, DESCRIPTOR_CLOSE, DESCRIPTOR_OPEN, Method, OBJECT_END, OBJECT_START};

const MAGIC: u32 = 0xCAFE_BABE;
const VERSION_SIZE: usize = 4;
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
pub(super) const TWO_BYTES: usize = 2;
const FIELD_HEADER: usize = 6;
pub(super) const THREE_BYTES: usize = 3;
pub(super) const FOUR_BYTES: usize = 4;
const EIGHT_BYTES: usize = 8;
const VISIBLE_ANNOTATIONS: &str = "RuntimeVisibleAnnotations";
const INVISIBLE_ANNOTATIONS: &str = "RuntimeInvisibleAnnotations";
const VISIBLE_PARAMETER_ANNOTATIONS: &str = "RuntimeVisibleParameterAnnotations";
const INVISIBLE_PARAMETER_ANNOTATIONS: &str = "RuntimeInvisibleParameterAnnotations";
const VISIBLE_TYPE_ANNOTATIONS: &str = "RuntimeVisibleTypeAnnotations";
const INVISIBLE_TYPE_ANNOTATIONS: &str = "RuntimeInvisibleTypeAnnotations";
const ACCESS_SYNTHETIC: u16 = 0x1000;
const ACCESS_BRIDGE: u16 = 0x0040;
const PATH_SEPARATOR: char = '/';
const NESTED_SEPARATOR: char = '$';
const QUALIFIER_SEPARATOR: char = '.';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassFile {
    pub name: String,
    pub superclass: Option<String>,
    pub interfaces: Vec<String>,
    pub annotations: Vec<String>,
    pub methods: Vec<Method>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed;

pub(super) struct Cursor<'bytes> {
    pub(super) bytes: &'bytes [u8],
    pub(super) offset: usize,
}

impl<'bytes> Cursor<'bytes> {
    pub(super) fn take(&mut self, length: usize) -> Result<&'bytes [u8], Malformed> {
        let end = self.offset.checked_add(length).ok_or(Malformed)?;
        let slice = self.bytes.get(self.offset..end).ok_or(Malformed)?;
        self.offset = end;
        Ok(slice)
    }

    pub(super) fn u8(&mut self) -> Result<u8, Malformed> {
        self.take(1)?.first().copied().ok_or(Malformed)
    }

    pub(super) fn u16(&mut self) -> Result<u16, Malformed> {
        let slice = self.take(TWO_BYTES)?;
        Ok(u16::from_be_bytes(slice.try_into().map_err(|_| Malformed)?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, Malformed> {
        let slice = self.take(FOUR_BYTES)?;
        Ok(u32::from_be_bytes(slice.try_into().map_err(|_| Malformed)?))
    }

    pub(super) fn length(&mut self) -> Result<usize, Malformed> {
        usize::try_from(self.u32()?).map_err(|_| Malformed)
    }
}

// El pool de constantes: sólo hacen falta los textos y las clases (que apuntan a un texto).
pub(super) struct Pool {
    texts: Vec<Option<String>>,
    classes: Vec<Option<u16>>,
}

impl Pool {
    pub(super) fn text(&self, index: u16) -> Result<&str, Malformed> {
        self.texts
            .get(usize::from(index))
            .and_then(Option::as_deref)
            .ok_or(Malformed)
    }

    fn class(&self, index: u16) -> Result<String, Malformed> {
        let name = self
            .classes
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(Malformed)?;
        Ok(binary_to_source(self.text(name)?))
    }
}

pub fn parse(bytes: &[u8]) -> Result<ClassFile, Malformed> {
    let mut cursor = Cursor { bytes, offset: 0 };
    if cursor.u32()? != MAGIC {
        return Err(Malformed);
    }
    cursor.take(VERSION_SIZE)?;
    let pool = constant_pool(&mut cursor)?;
    cursor.u16()?;
    let name = pool.class(cursor.u16()?)?;
    let superclass_index = cursor.u16()?;
    let superclass = (superclass_index != 0)
        .then(|| pool.class(superclass_index))
        .transpose()?;
    let interface_count = cursor.u16()?;
    let interfaces = (0..interface_count)
        .map(|_| pool.class(cursor.u16()?))
        .collect::<Result<Vec<_>, _>>()?;
    let field_count = cursor.u16()?;
    for _ in 0..field_count {
        cursor.take(FIELD_HEADER)?;
        skip_attributes(&mut cursor)?;
    }
    let method_count = cursor.u16()?;
    let mut methods = Vec::new();
    for _ in 0..method_count {
        methods.extend(method(&mut cursor, &pool)?);
    }
    let annotations = class_annotations(&mut cursor, &pool)?;
    Ok(ClassFile {
        name,
        superclass,
        interfaces,
        annotations,
        methods,
    })
}

fn constant_pool(cursor: &mut Cursor<'_>) -> Result<Pool, Malformed> {
    let count = usize::from(cursor.u16()?);
    let mut pool = Pool {
        texts: vec![None; count],
        classes: vec![None; count],
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
            INTEGER
            | FLOAT
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

fn method(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<Option<Method>, Malformed> {
    let access = cursor.u16()?;
    let name = pool.text(cursor.u16()?)?.to_owned();
    let descriptor = pool.text(cursor.u16()?)?.to_owned();
    let mut parameter_annotations = vec![Vec::new(); parameter_count(&descriptor)];
    let mut return_annotations = Vec::new();
    let attribute_count = cursor.u16()?;
    for _ in 0..attribute_count {
        let attribute = pool.text(cursor.u16()?)?;
        let length = cursor.length()?;
        let mut body = Cursor {
            bytes: cursor.take(length)?,
            offset: 0,
        };
        match attribute {
            VISIBLE_PARAMETER_ANNOTATIONS | INVISIBLE_PARAMETER_ANNOTATIONS => {
                let parameters = usize::from(body.u8()?);
                let shift = parameter_annotations.len().saturating_sub(parameters);
                for parameter in 0..parameters {
                    let found = annotation_list(&mut body, pool)?;
                    add_to(&mut parameter_annotations, parameter.saturating_add(shift), found);
                }
            }
            VISIBLE_TYPE_ANNOTATIONS | INVISIBLE_TYPE_ANNOTATIONS => {
                for (place, annotation) in type_annotations(&mut body, pool)? {
                    place_annotation(
                        &mut parameter_annotations,
                        &mut return_annotations,
                        place,
                        annotation,
                    );
                }
            }
            VISIBLE_ANNOTATIONS | INVISIBLE_ANNOTATIONS => {
                return_annotations.extend(annotation_list(&mut body, pool)?);
            }
            _ => {}
        }
    }
    let generated = access & (ACCESS_SYNTHETIC | ACCESS_BRIDGE) != 0;
    Ok((!generated).then_some(Method {
        name,
        descriptor,
        access,
        parameter_annotations,
        return_annotations,
    }))
}

fn place_annotation(
    parameters: &mut [Vec<String>],
    returned: &mut Vec<String>,
    place: TypeTarget,
    annotation: String,
) {
    match place {
        TypeTarget::Parameter(parameter) => add_to(parameters, parameter, vec![annotation]),
        TypeTarget::Return => returned.push(annotation),
    }
}

fn add_to(parameters: &mut [Vec<String>], index: usize, found: Vec<String>) {
    if let Some(slot) = parameters.get_mut(index) {
        slot.extend(found);
    }
}

fn class_annotations(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<Vec<String>, Malformed> {
    let mut annotations = Vec::new();
    let attribute_count = cursor.u16()?;
    for _ in 0..attribute_count {
        let attribute = pool.text(cursor.u16()?)?;
        let length = cursor.length()?;
        let mut body = Cursor {
            bytes: cursor.take(length)?,
            offset: 0,
        };
        if attribute == VISIBLE_ANNOTATIONS || attribute == INVISIBLE_ANNOTATIONS {
            annotations.extend(annotation_list(&mut body, pool)?);
        }
    }
    Ok(annotations)
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

// Cuántos parámetros declara un descriptor como `(ILjava/lang/String;[J)V`.
pub fn parameter_count(descriptor: &str) -> usize {
    let bytes = descriptor.as_bytes();
    let Some(open) = bytes.iter().position(|byte| *byte == DESCRIPTOR_OPEN) else {
        return 0;
    };
    let mut count = 0_usize;
    let mut index = open.saturating_add(1);
    while let Some(&byte) = bytes.get(index) {
        match byte {
            DESCRIPTOR_CLOSE => break,
            ARRAY => {}
            OBJECT_START => {
                index = bytes
                    .get(index..)
                    .and_then(|rest| rest.iter().position(|letter| *letter == OBJECT_END))
                    .map_or(bytes.len(), |end| index.saturating_add(end));
                count = count.saturating_add(1);
            }
            _ => count = count.saturating_add(1),
        }
        index = index.saturating_add(1);
    }
    count
}

pub(super) fn descriptor_to_source(descriptor: &str) -> String {
    let inner = descriptor
        .strip_prefix(char::from(OBJECT_START))
        .and_then(|rest| rest.strip_suffix(char::from(OBJECT_END)))
        .unwrap_or(descriptor);
    binary_to_source(inner)
}

// `org/springframework/web/filter/OncePerRequestFilter` y `Outer$Inner` en notación de Java.
pub fn binary_to_source(binary: &str) -> String {
    binary
        .replace(PATH_SEPARATOR, &QUALIFIER_SEPARATOR.to_string())
        .replace(NESTED_SEPARATOR, &QUALIFIER_SEPARATOR.to_string())
}
