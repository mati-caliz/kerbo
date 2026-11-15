// Lo que kerbo necesita de un `.class`, sin bytecode: el nombre, los padres, las anotaciones de la
// clase y, por método, el nombre, el descriptor, la visibilidad y las anotaciones de cada
// parámetro (las de declaración y las de tipo, como el `@Nullable` de JSpecify).

use super::annotation_defaults::default_value as annotation_default;
use super::annotations::{TypeTarget, annotation_list, type_annotations};
pub(super) use super::constant_pool::Pool;
use super::constant_pool::constant_pool;
use super::method::{ARRAY, DESCRIPTOR_CLOSE, DESCRIPTOR_OPEN, Method, OBJECT_END, OBJECT_START};

const MAGIC: u32 = 0xCAFE_BABE;
const VERSION_SIZE: usize = 4;
pub(super) const TWO_BYTES: usize = 2;
pub(super) const THREE_BYTES: usize = 3;
pub(super) const FOUR_BYTES: usize = 4;
const VISIBLE_ANNOTATIONS: &str = "RuntimeVisibleAnnotations";
const INVISIBLE_ANNOTATIONS: &str = "RuntimeInvisibleAnnotations";
const VISIBLE_PARAMETER_ANNOTATIONS: &str = "RuntimeVisibleParameterAnnotations";
const INVISIBLE_PARAMETER_ANNOTATIONS: &str = "RuntimeInvisibleParameterAnnotations";
const VISIBLE_TYPE_ANNOTATIONS: &str = "RuntimeVisibleTypeAnnotations";
const INVISIBLE_TYPE_ANNOTATIONS: &str = "RuntimeInvisibleTypeAnnotations";
const EXCEPTIONS: &str = "Exceptions";
const SIGNATURE: &str = "Signature";
const ANNOTATION_DEFAULT: &str = "AnnotationDefault";
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
    pub fields: Vec<ClassField>,
    pub signature: Option<String>,
}

// Un campo: el nombre, el tipo borrado (`Ljava/nio/charset/Charset;`) y el genérico, si lo tiene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassField {
    pub name: String,
    pub descriptor: String,
    pub signature: Option<String>,
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
    let fields = (0..field_count)
        .map(|_| field(&mut cursor, &pool))
        .collect::<Result<Vec<_>, _>>()?;
    let method_count = cursor.u16()?;
    let mut methods = Vec::new();
    for _ in 0..method_count {
        methods.extend(method(&mut cursor, &pool)?);
    }
    let ClassAttributes {
        annotations,
        signature,
    } = class_attributes(&mut cursor, &pool)?;
    Ok(ClassFile {
        name,
        superclass,
        interfaces,
        annotations,
        methods,
        fields,
        signature,
    })
}

fn field(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<ClassField, Malformed> {
    cursor.u16()?;
    let name = pool.text(cursor.u16()?)?.to_owned();
    let descriptor = pool.text(cursor.u16()?)?.to_owned();
    let mut signature = None;
    let attribute_count = cursor.u16()?;
    for _ in 0..attribute_count {
        let attribute = pool.text(cursor.u16()?)?;
        let length = cursor.length()?;
        let mut body = Cursor {
            bytes: cursor.take(length)?,
            offset: 0,
        };
        if attribute == SIGNATURE {
            signature = Some(pool.text(body.u16()?)?.to_owned());
        }
    }
    Ok(ClassField {
        name,
        descriptor,
        signature,
    })
}

fn method(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<Option<Method>, Malformed> {
    let access = cursor.u16()?;
    let name = pool.text(cursor.u16()?)?.to_owned();
    let descriptor = pool.text(cursor.u16()?)?.to_owned();
    let mut parameter_annotations = vec![Vec::new(); parameter_count(&descriptor)];
    let mut return_annotations = Vec::new();
    let mut exceptions = Vec::new();
    let mut signature = None;
    let mut default_value = None;
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
            EXCEPTIONS => {
                let count = body.u16()?;
                for _ in 0..count {
                    exceptions.push(pool.class(body.u16()?)?);
                }
            }
            SIGNATURE => signature = Some(pool.text(body.u16()?)?.to_owned()),
            ANNOTATION_DEFAULT => default_value = annotation_default(&mut body, pool)?,
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
        exceptions,
        signature,
        default_value,
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

struct ClassAttributes {
    annotations: Vec<String>,
    signature: Option<String>,
}

fn class_attributes(cursor: &mut Cursor<'_>, pool: &Pool) -> Result<ClassAttributes, Malformed> {
    let mut annotations = Vec::new();
    let mut signature = None;
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
        } else if attribute == SIGNATURE {
            signature = Some(pool.text(body.u16()?)?.to_owned());
        }
    }
    Ok(ClassAttributes {
        annotations,
        signature,
    })
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
