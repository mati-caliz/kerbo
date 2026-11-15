use super::constant_pool::{DOUBLE, LONG, constant_size};

const CLASS_HEADER_SIZE: usize = 8;
const CONSTANT_UTF8: u8 = 1;
const EXTERNALIZED_STRING: u8 = 23;
const EXTERNALIZED_DESCRIPTOR: u8 = 25;
const COMPRESSED_FLAG: u8 = 0b1000_0000;
const LENGTH_SHIFT: u8 = 5;
const LENGTH_MASK: u8 = 0b11;
const VALUE_MASK: u8 = 0b1_1111;
const FULL_INT_SIZE: usize = 4;
const CLASS_MARK: u8 = b'L';
const PACKAGE_SEPARATOR: u8 = b'/';
const STRING_END: u8 = 0;

// Rearma una clase de un runtime de `jlink --compress=1` (`compact-cp`, el `StringSharingDecompressor`
// del JDK): los textos del constant pool vienen como índices a la tabla de textos de la imagen, y los
// descriptores (`(Ljava/lang/String;)V`) con el esqueleto y cada clase por separado. El resto del
// archivo queda igual.
pub(super) fn restore_shared_strings(stored: &[u8], strings: &[u8]) -> Option<Vec<u8>> {
    let mut reader = Reader {
        bytes: stored,
        position: 0,
    };
    let mut restored = Vec::with_capacity(stored.len());
    restored.extend_from_slice(reader.take(CLASS_HEADER_SIZE)?);
    let count_bytes = reader.take(2)?;
    restored.extend_from_slice(count_bytes);
    let count = u16::from_be_bytes(count_bytes.try_into().ok()?);
    let mut index = 1_u16;
    while index < count {
        let tag = reader.byte()?;
        match tag {
            CONSTANT_UTF8 => {
                let length = reader.take(2)?;
                let size = usize::from(u16::from_be_bytes(length.try_into().ok()?));
                push_utf8(&mut restored, reader.take(size)?)?;
            }
            EXTERNALIZED_STRING => {
                let text = string_at(strings, reader.compressed_int()?)?;
                push_utf8(&mut restored, text)?;
            }
            EXTERNALIZED_DESCRIPTOR => {
                let descriptor = descriptor(&mut reader, strings)?;
                push_utf8(&mut restored, &descriptor)?;
            }
            _ => {
                restored.push(tag);
                restored.extend_from_slice(reader.take(constant_size(tag)?)?);
                if tag == LONG || tag == DOUBLE {
                    index = index.checked_add(1)?;
                }
            }
        }
        index = index.checked_add(1)?;
    }
    restored.extend_from_slice(stored.get(reader.position..)?);
    Some(restored)
}

// El esqueleto del descriptor con una `L` por clase, y por cada una el paquete (cero si no tiene) y
// el nombre simple.
fn descriptor(reader: &mut Reader<'_>, strings: &[u8]) -> Option<Vec<u8>> {
    let skeleton = string_at(strings, reader.compressed_int()?)?;
    let indexes_size = usize::try_from(reader.compressed_int()?).ok()?;
    let mut indexes = Reader {
        bytes: reader.take(indexes_size)?,
        position: 0,
    };
    let mut rebuilt = Vec::new();
    for &character in skeleton {
        rebuilt.push(character);
        if character != CLASS_MARK {
            continue;
        }
        let package = indexes.compressed_int()?;
        if package != 0 {
            rebuilt.extend_from_slice(string_at(strings, package)?);
            rebuilt.push(PACKAGE_SEPARATOR);
        }
        rebuilt.extend_from_slice(string_at(strings, indexes.compressed_int()?)?);
    }
    Some(rebuilt)
}

fn push_utf8(restored: &mut Vec<u8>, text: &[u8]) -> Option<()> {
    restored.push(CONSTANT_UTF8);
    restored.extend_from_slice(&u16::try_from(text.len()).ok()?.to_be_bytes());
    restored.extend_from_slice(text);
    Some(())
}

fn string_at(strings: &[u8], offset: u32) -> Option<&[u8]> {
    let rest = strings.get(usize::try_from(offset).ok()?..)?;
    let end = rest.iter().position(|byte| *byte == STRING_END)?;
    rest.get(..end)
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    position: usize,
}

impl<'bytes> Reader<'bytes> {
    fn take(&mut self, size: usize) -> Option<&'bytes [u8]> {
        let end = self.position.checked_add(size)?;
        let taken = self.bytes.get(self.position..end)?;
        self.position = end;
        Some(taken)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1)?.first().copied()
    }

    // `CompressIndexes.readInt`: con el bit alto, el largo (1 a 3 bytes) va en los dos bits que siguen
    // y el valor arranca en los cinco de abajo; sin él, son cuatro bytes en big endian.
    fn compressed_int(&mut self) -> Option<u32> {
        let header = self.byte()?;
        let (size, mut value) = if header & COMPRESSED_FLAG == 0 {
            (FULL_INT_SIZE, u32::from(header))
        } else {
            (
                usize::from((header >> LENGTH_SHIFT) & LENGTH_MASK),
                u32::from(header & VALUE_MASK),
            )
        };
        for _ in 1..size {
            value = (value << u8::BITS) | u32::from(self.byte()?);
        }
        Some(value)
    }
}
