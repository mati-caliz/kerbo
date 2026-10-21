use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::{Path, PathBuf};

const END_SIGNATURE: u32 = 0x0605_4b50;
const ZIP64_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
const ZIP64_END_SIGNATURE: u32 = 0x0606_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const END_SIZE: usize = 22;
const MAX_COMMENT: usize = 0xffff;
const ZIP64_LOCATOR_SIZE: usize = 20;
const ZIP64_END_SIZE: usize = 56;
const CENTRAL_HEADER_SIZE: usize = 46;
const LOCAL_HEADER_SIZE: usize = 30;
const SATURATED_32: u32 = 0xffff_ffff;
const SATURATED_16: u16 = 0xffff;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;
const ZIP64_EXTRA: u16 = 0x0001;
const EXTRA_HEADER: usize = 4;
const EXTRA_SIZE_AT: usize = 2;
const END_COUNT_AT: usize = 10;
const END_DIRECTORY_SIZE_AT: usize = 12;
const END_OFFSET_AT: usize = 16;
const LOCATOR_END_AT: usize = 8;
const ZIP64_COUNT_AT: usize = 32;
const ZIP64_DIRECTORY_SIZE_AT: usize = 40;
const ZIP64_OFFSET_AT: usize = 48;
const CENTRAL_METHOD_AT: usize = 10;
const CENTRAL_COMPRESSED_AT: usize = 20;
const CENTRAL_UNCOMPRESSED_AT: usize = 24;
const CENTRAL_NAME_LENGTH_AT: usize = 28;
const CENTRAL_EXTRA_LENGTH_AT: usize = 30;
const CENTRAL_COMMENT_LENGTH_AT: usize = 32;
const CENTRAL_LOCAL_HEADER_AT: usize = 42;
const LOCAL_NAME_LENGTH_AT: usize = 26;
const LOCAL_EXTRA_LENGTH_AT: usize = 28;

// Un zip (un jar) del que sólo se leyó el índice: las entradas se sacan del disco cuando hacen
// falta, así un classpath de cientos de jars no se carga entero en memoria.
pub struct Archive {
    source: Source,
    entries: HashMap<String, Entry>,
}

struct Source {
    path: PathBuf,
}

impl Source {
    fn length(&self) -> Option<usize> {
        usize::try_from(self.path.metadata().ok()?.len()).ok()
    }

    fn range(&self, offset: usize, length: usize) -> Option<Vec<u8>> {
        let mut file = File::open(&self.path).ok()?;
        file.seek(SeekFrom::Start(u64::try_from(offset).ok()?)).ok()?;
        let mut buffer = vec![0; length];
        file.read_exact(&mut buffer).ok()?;
        Some(buffer)
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    method: u16,
    compressed: usize,
    local_header: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable;

impl Archive {
    pub fn open(path: &Path) -> Result<Self, Unreadable> {
        Self::index(Source {
            path: path.to_path_buf(),
        })
    }

    fn index(source: Source) -> Result<Self, Unreadable> {
        let (count, directory) = central_directory(&source)?;
        let mut entries = HashMap::with_capacity(count);
        let mut offset = 0;
        for _ in 0..count {
            let (name, entry, next) = central_entry(&directory, offset)?;
            entries.insert(name, entry);
            offset = next;
        }
        Ok(Self { source, entries })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let entry = self.entries.get(name)?;
        let header = self.source.range(entry.local_header, LOCAL_HEADER_SIZE)?;
        if read_u32(&header, 0)? != LOCAL_SIGNATURE {
            return None;
        }
        let name_length = usize::from(read_u16(&header, LOCAL_NAME_LENGTH_AT)?);
        let extra_length = usize::from(read_u16(&header, LOCAL_EXTRA_LENGTH_AT)?);
        let start = entry
            .local_header
            .checked_add(LOCAL_HEADER_SIZE)?
            .checked_add(name_length)?
            .checked_add(extra_length)?;
        let data = self.source.range(start, entry.compressed)?;
        match entry.method {
            STORED => Some(data),
            DEFLATED => miniz_oxide::inflate::decompress_to_vec(&data).ok(),
            _ => None,
        }
    }
}

// La cantidad de entradas y los bytes del directorio central, leyendo el final del archivo (y el
// de zip64 si el normal no alcanza).
fn central_directory(source: &Source) -> Result<(usize, Vec<u8>), Unreadable> {
    let length = source.length().ok_or(Unreadable)?;
    let tail_length = length.min(
        END_SIZE
            .saturating_add(MAX_COMMENT)
            .saturating_add(ZIP64_LOCATOR_SIZE),
    );
    let tail_start = length.saturating_sub(tail_length);
    let tail = source.range(tail_start, tail_length).ok_or(Unreadable)?;
    let end = (0..=tail.len().saturating_sub(END_SIZE))
        .rev()
        .find(|offset| read_u32(&tail, *offset) == Some(END_SIGNATURE))
        .ok_or(Unreadable)?;
    let field16 = |at: usize| read_u16(&tail, end.saturating_add(at)).ok_or(Unreadable);
    let field32 = |at: usize| read_u32(&tail, end.saturating_add(at)).ok_or(Unreadable);
    let count = field16(END_COUNT_AT)?;
    let size = field32(END_DIRECTORY_SIZE_AT)?;
    let offset = field32(END_OFFSET_AT)?;
    let (count, size, offset) = if count == SATURATED_16 || size == SATURATED_32 || offset == SATURATED_32 {
        zip64_directory(source, &tail, end)?
    } else {
        (
            usize::from(count),
            to_usize(u64::from(size))?,
            to_usize(u64::from(offset))?,
        )
    };
    let directory = source.range(offset, size).ok_or(Unreadable)?;
    Ok((count, directory))
}

fn zip64_directory(source: &Source, tail: &[u8], end: usize) -> Result<(usize, usize, usize), Unreadable> {
    let locator = end.checked_sub(ZIP64_LOCATOR_SIZE).ok_or(Unreadable)?;
    if read_u32(tail, locator) != Some(ZIP64_LOCATOR_SIGNATURE) {
        return Err(Unreadable);
    }
    let record_at = to_usize(read_u64(tail, locator.saturating_add(LOCATOR_END_AT)).ok_or(Unreadable)?)?;
    let record = source.range(record_at, ZIP64_END_SIZE).ok_or(Unreadable)?;
    if read_u32(&record, 0) != Some(ZIP64_END_SIGNATURE) {
        return Err(Unreadable);
    }
    let field64 = |at: usize| read_u64(&record, at).ok_or(Unreadable).and_then(to_usize);
    Ok((
        field64(ZIP64_COUNT_AT)?,
        field64(ZIP64_DIRECTORY_SIZE_AT)?,
        field64(ZIP64_OFFSET_AT)?,
    ))
}

fn central_entry(directory: &[u8], offset: usize) -> Result<(String, Entry, usize), Unreadable> {
    if read_u32(directory, offset) != Some(CENTRAL_SIGNATURE) {
        return Err(Unreadable);
    }
    let field16 = |at: usize| read_u16(directory, offset.saturating_add(at)).ok_or(Unreadable);
    let field32 = |at: usize| read_u32(directory, offset.saturating_add(at)).ok_or(Unreadable);
    let method = field16(CENTRAL_METHOD_AT)?;
    let mut compressed = u64::from(field32(CENTRAL_COMPRESSED_AT)?);
    let uncompressed = field32(CENTRAL_UNCOMPRESSED_AT)?;
    let name_length = usize::from(field16(CENTRAL_NAME_LENGTH_AT)?);
    let extra_length = usize::from(field16(CENTRAL_EXTRA_LENGTH_AT)?);
    let comment_length = usize::from(field16(CENTRAL_COMMENT_LENGTH_AT)?);
    let mut local_header = u64::from(field32(CENTRAL_LOCAL_HEADER_AT)?);
    let name_start = offset.saturating_add(CENTRAL_HEADER_SIZE);
    let extra_start = name_start.saturating_add(name_length);
    let name = directory.get(name_start..extra_start).ok_or(Unreadable)?;
    let extra = directory
        .get(extra_start..extra_start.saturating_add(extra_length))
        .ok_or(Unreadable)?;
    if compressed == u64::from(SATURATED_32) || local_header == u64::from(SATURATED_32) {
        let mut values = zip64_values(extra).into_iter();
        if uncompressed == SATURATED_32 {
            values.next();
        }
        if compressed == u64::from(SATURATED_32) {
            compressed = values.next().ok_or(Unreadable)?;
        }
        if local_header == u64::from(SATURATED_32) {
            local_header = values.next().ok_or(Unreadable)?;
        }
    }
    let next = extra_start
        .saturating_add(extra_length)
        .saturating_add(comment_length);
    Ok((
        String::from_utf8_lossy(name).into_owned(),
        Entry {
            method,
            compressed: to_usize(compressed)?,
            local_header: to_usize(local_header)?,
        },
        next,
    ))
}

// Los valores de 64 bits del campo extra de zip64, en el orden en que los pide el formato.
fn zip64_values(extra: &[u8]) -> Vec<u64> {
    let mut offset = 0;
    while let (Some(id), Some(size)) = (
        read_u16(extra, offset),
        read_u16(extra, offset.saturating_add(EXTRA_SIZE_AT)),
    ) {
        let data = offset.saturating_add(EXTRA_HEADER);
        if id == ZIP64_EXTRA {
            return (0..usize::from(size) / size_of::<u64>())
                .filter_map(|index| {
                    read_u64(extra, data.saturating_add(index.saturating_mul(size_of::<u64>())))
                })
                .collect();
        }
        offset = data.saturating_add(usize::from(size));
    }
    Vec::new()
}

fn to_usize(value: u64) -> Result<usize, Unreadable> {
    usize::try_from(value).map_err(|_| Unreadable)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset.checked_add(size_of::<u16>())?)?;
    Some(u16::from_le_bytes(slice.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset.checked_add(size_of::<u32>())?)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset.checked_add(size_of::<u64>())?)?;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}
