use std::collections::HashMap;
use std::mem::size_of;
use std::path::{Path, PathBuf};

use super::string_sharing::restore_shared_strings;
use super::zip::read_range;

const MAGIC: u32 = 0xCAFE_DADA;
const HEADER_SLOTS: usize = 7;
const TABLE_LENGTH_SLOT: usize = 4;
const LOCATIONS_SIZE_SLOT: usize = 5;
const STRINGS_SIZE_SLOT: usize = 6;
const ATTRIBUTE_END: u8 = 0;
const ATTRIBUTE_PARENT: u8 = 2;
const ATTRIBUTE_BASE: u8 = 3;
const ATTRIBUTE_EXTENSION: u8 = 4;
const ATTRIBUTE_OFFSET: u8 = 5;
const ATTRIBUTE_COMPRESSED: u8 = 6;
const ATTRIBUTE_UNCOMPRESSED: u8 = 7;
const ATTRIBUTE_COUNT: usize = 8;
const KIND_SHIFT: u8 = 3;
const LENGTH_MASK: u8 = 0b111;
const CLASS_EXTENSION: &str = "class";
const STRING_END: u8 = 0;
const COMPRESSED_MAGIC: u32 = 0xCAFE_FAFA;
const COMPRESSED_HEADER_SIZE: usize = 29;
const UNCOMPRESSED_SIZE_AT: usize = 12;
const DECOMPRESSOR_NAME_AT: usize = 20;
const ZIP_DECOMPRESSOR: &str = "zip";
const STRING_SHARING_DECOMPRESSOR: &str = "compact-cp";

#[derive(Debug, Clone, Copy)]
struct Resource {
    offset: usize,
    stored: usize,
    compressed: bool,
}

// `lib/modules` del JDK: el formato jimage, con el índice en memoria y cada clase leída del disco a
// pedido (el archivo pesa más de 100 MB). Una clase de un runtime de `jlink --compress` trae una o
// más cabeceras (`CompressedResourceHeader`) con el descompresor que la abre: `zip` (zlib, el de
// `--compress=2` y `zip-N`) o `compact-cp` (el viejo `--compress=1`, que reparte los textos del
// constant pool en la tabla de la imagen).
#[derive(Debug)]
pub struct ModulesImage {
    path: PathBuf,
    data_start: usize,
    resources: HashMap<String, Resource>,
    strings: Vec<u8>,
}

impl ModulesImage {
    pub fn open(path: &Path) -> Option<Self> {
        let header_size = HEADER_SLOTS.checked_mul(size_of::<u32>())?;
        let header = read_range(path, 0, header_size)?;
        if read_u32(&header, 0)? != MAGIC {
            return None;
        }
        let slot =
            |index: usize| usize::try_from(read_u32(&header, index.checked_mul(size_of::<u32>())?)?).ok();
        let table_length = slot(TABLE_LENGTH_SLOT)?;
        let locations_size = slot(LOCATIONS_SIZE_SLOT)?;
        let strings_size = slot(STRINGS_SIZE_SLOT)?;
        let table_size = table_length.checked_mul(size_of::<u32>())?;
        let index_size = table_size
            .checked_add(table_size)?
            .checked_add(locations_size)?
            .checked_add(strings_size)?;
        let index = read_range(path, header_size, index_size)?;
        let offsets = index.get(table_size..table_size.checked_add(table_size)?)?;
        let locations_start = table_size.checked_add(table_size)?;
        let strings_start = locations_start.checked_add(locations_size)?;
        let locations = index.get(locations_start..strings_start)?;
        let strings = index.get(strings_start..)?;
        let resources = (0..table_length)
            .filter_map(|position| {
                let location = read_u32(offsets, position.checked_mul(size_of::<u32>())?)?;
                resource_at(locations.get(usize::try_from(location).ok()?..)?, strings)
            })
            .collect();
        Some(Self {
            path: path.to_path_buf(),
            data_start: header_size.checked_add(index_size)?,
            resources,
            strings: strings.to_vec(),
        })
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.resources.contains_key(entry)
    }

    pub fn read(&self, entry: &str) -> Option<Vec<u8>> {
        let resource = self.resources.get(entry)?;
        let stored = read_range(
            &self.path,
            self.data_start.checked_add(resource.offset)?,
            resource.stored,
        )?;
        if resource.compressed {
            decompress(stored, &self.strings)
        } else {
            Some(stored)
        }
    }
}

// Saca las cabeceras de compresión de a una, aplicando el descompresor que nombra cada una.
fn decompress(mut bytes: Vec<u8>, strings: &[u8]) -> Option<Vec<u8>> {
    while read_u32(&bytes, 0) == Some(COMPRESSED_MAGIC) {
        let uncompressed = usize::try_from(read_u64(&bytes, UNCOMPRESSED_SIZE_AT)?).ok()?;
        let name = text_at(strings, u64::from(read_u32(&bytes, DECOMPRESSOR_NAME_AT)?))?;
        let payload = bytes.get(COMPRESSED_HEADER_SIZE..)?;
        bytes = match name {
            ZIP_DECOMPRESSOR => miniz_oxide::inflate::decompress_to_vec_zlib(payload).ok()?,
            STRING_SHARING_DECOMPRESSOR => restore_shared_strings(payload, strings)?,
            _ => return None,
        };
        if bytes.len() != uncompressed {
            return None;
        }
    }
    Some(bytes)
}

fn resource_at(location: &[u8], strings: &[u8]) -> Option<(String, Resource)> {
    let attributes = attributes_of(location)?;
    let attribute = |kind: u8| attributes.get(usize::from(kind)).copied();
    if text_at(strings, attribute(ATTRIBUTE_EXTENSION)?)? != CLASS_EXTENSION {
        return None;
    }
    let compressed = attribute(ATTRIBUTE_COMPRESSED)?;
    let parent = text_at(strings, attribute(ATTRIBUTE_PARENT)?)?;
    let base = text_at(strings, attribute(ATTRIBUTE_BASE)?)?;
    Some((
        format!("{parent}/{base}.{CLASS_EXTENSION}"),
        Resource {
            offset: usize::try_from(attribute(ATTRIBUTE_OFFSET)?).ok()?,
            stored: usize::try_from(if compressed == 0 {
                attribute(ATTRIBUTE_UNCOMPRESSED)?
            } else {
                compressed
            })
            .ok()?,
            compressed: compressed != 0,
        },
    ))
}

// Cada atributo es un byte con el tipo y el largo, y después el valor en big endian.
fn attributes_of(location: &[u8]) -> Option<[u64; ATTRIBUTE_COUNT]> {
    let mut attributes = [0; ATTRIBUTE_COUNT];
    let mut position = 0_usize;
    loop {
        let head = *location.get(position)?;
        let kind = head >> KIND_SHIFT;
        if kind == ATTRIBUTE_END {
            return Some(attributes);
        }
        let length = usize::from(head & LENGTH_MASK).checked_add(1)?;
        let start = position.checked_add(1)?;
        position = start.checked_add(length)?;
        let value = location
            .get(start..position)?
            .iter()
            .fold(0_u64, |total, byte| (total << u8::BITS) | u64::from(*byte));
        *attributes.get_mut(usize::from(kind))? = value;
    }
}

fn text_at(strings: &[u8], offset: u64) -> Option<&str> {
    let rest = strings.get(usize::try_from(offset).ok()?..)?;
    let end = rest.iter().position(|byte| *byte == STRING_END)?;
    std::str::from_utf8(rest.get(..end)?).ok()
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset.checked_add(size_of::<u32>())?)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset.checked_add(size_of::<u64>())?)?;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}
