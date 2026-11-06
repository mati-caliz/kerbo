use std::collections::HashMap;
use std::mem::size_of;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Copy)]
struct Resource {
    offset: usize,
    size: usize,
}

// `lib/modules` del JDK: el formato jimage, con el índice en memoria y cada clase leída del disco a
// pedido (el archivo pesa más de 100 MB). Una clase comprimida (un runtime de `jlink --compress`) no
// se puede leer sin los descompresores del JDK: queda afuera, y sus llamadas no se resuelven.
#[derive(Debug)]
pub struct ModulesImage {
    path: PathBuf,
    data_start: usize,
    resources: HashMap<String, Resource>,
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
        })
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.resources.contains_key(entry)
    }

    pub fn read(&self, entry: &str) -> Option<Vec<u8>> {
        let resource = self.resources.get(entry)?;
        read_range(
            &self.path,
            self.data_start.checked_add(resource.offset)?,
            resource.size,
        )
    }
}

fn resource_at(location: &[u8], strings: &[u8]) -> Option<(String, Resource)> {
    let attributes = attributes_of(location)?;
    let attribute = |kind: u8| attributes.get(usize::from(kind)).copied();
    if text_at(strings, attribute(ATTRIBUTE_EXTENSION)?)? != CLASS_EXTENSION
        || attribute(ATTRIBUTE_COMPRESSED)? != 0
    {
        return None;
    }
    let parent = text_at(strings, attribute(ATTRIBUTE_PARENT)?)?;
    let base = text_at(strings, attribute(ATTRIBUTE_BASE)?)?;
    Some((
        format!("{parent}/{base}.{CLASS_EXTENSION}"),
        Resource {
            offset: usize::try_from(attribute(ATTRIBUTE_OFFSET)?).ok()?,
            size: usize::try_from(attribute(ATTRIBUTE_UNCOMPRESSED)?).ok()?,
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
