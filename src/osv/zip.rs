use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const END_SIGNATURE: u32 = 0x0605_4b50;
const ZIP64_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
const ZIP64_END_SIGNATURE: u32 = 0x0606_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const END_LENGTH: usize = 22;
const MAX_COMMENT_LENGTH: usize = 0xffff;
const END_ENTRIES_AT: usize = 10;
const END_SIZE_AT: usize = 12;
const END_START_AT: usize = 16;
const LOCATOR_LENGTH: usize = 20;
const LOCATOR_RECORD_AT: usize = 8;
const ZIP64_END_LENGTH: usize = 56;
const ZIP64_SIZE_AT: usize = 40;
const ZIP64_START_AT: usize = 48;
const CENTRAL_LENGTH: usize = 46;
const CENTRAL_METHOD_AT: usize = 10;
const CENTRAL_COMPRESSED_AT: usize = 20;
const CENTRAL_UNCOMPRESSED_AT: usize = 24;
const CENTRAL_NAME_AT: usize = 28;
const CENTRAL_EXTRA_AT: usize = 30;
const CENTRAL_COMMENT_AT: usize = 32;
const CENTRAL_OFFSET_AT: usize = 42;
const LOCAL_LENGTH: usize = 30;
const LOCAL_NAME_AT: usize = 26;
const LOCAL_EXTRA_AT: usize = 28;
const EXTRA_HEADER_LENGTH: usize = 4;
const EXTRA_LENGTH_AT: usize = 2;
const ZIP64_EXTRA_ID: u16 = 0x0001;
const SATURATED_16: u16 = u16::MAX;
const SATURATED_32: u32 = u32::MAX;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;
const NOT_A_ZIP: &str = "el archivo no es un zip válido";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub offset: u64,
    pub size: u64,
    pub method: u16,
}

#[derive(Debug)]
pub struct Archive {
    file: File,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let slice = bytes.get(at..at.checked_add(size_of::<u16>())?)?;
    Some(u16::from_le_bytes(slice.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(size_of::<u32>())?)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    let slice = bytes.get(at..at.checked_add(size_of::<u64>())?)?;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}

fn invalid() -> String {
    NOT_A_ZIP.to_owned()
}

fn wide(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn narrow(value: u64) -> Result<usize, String> {
    usize::try_from(value).map_err(|error| error.to_string())
}

impl Archive {
    pub const fn open(file: File) -> Self {
        Self { file }
    }

    fn read_at(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, String> {
        let mut buffer = vec![0; length];
        self.file
            .seek(SeekFrom::Start(offset))
            .and_then(|_| self.file.read_exact(&mut buffer))
            .map_err(|error| error.to_string())?;
        Ok(buffer)
    }

    fn directory_span(&mut self) -> Result<(u64, u64), String> {
        let length = self.file.metadata().map_err(|error| error.to_string())?.len();
        let tail_length = length.min(wide(END_LENGTH.saturating_add(MAX_COMMENT_LENGTH)));
        let tail_start = length.saturating_sub(tail_length);
        let tail = self.read_at(tail_start, narrow(tail_length)?)?;
        let end = (0..tail.len().saturating_sub(END_LENGTH.saturating_sub(1)))
            .rev()
            .find(|&at| u32_at(&tail, at) == Some(END_SIGNATURE))
            .ok_or_else(invalid)?;
        let entries = u16_at(&tail, end.saturating_add(END_ENTRIES_AT)).ok_or_else(invalid)?;
        let size = u32_at(&tail, end.saturating_add(END_SIZE_AT)).ok_or_else(invalid)?;
        let start = u32_at(&tail, end.saturating_add(END_START_AT)).ok_or_else(invalid)?;
        if entries != SATURATED_16 && size != SATURATED_32 && start != SATURATED_32 {
            return Ok((u64::from(start), u64::from(size)));
        }
        let end_offset = tail_start.saturating_add(wide(end));
        let locator = self.read_at(end_offset.saturating_sub(wide(LOCATOR_LENGTH)), LOCATOR_LENGTH)?;
        if u32_at(&locator, 0) != Some(ZIP64_LOCATOR_SIGNATURE) {
            return Err(invalid());
        }
        let record_offset = u64_at(&locator, LOCATOR_RECORD_AT).ok_or_else(invalid)?;
        let record = self.read_at(record_offset, ZIP64_END_LENGTH)?;
        if u32_at(&record, 0) != Some(ZIP64_END_SIGNATURE) {
            return Err(invalid());
        }
        Ok((
            u64_at(&record, ZIP64_START_AT).ok_or_else(invalid)?,
            u64_at(&record, ZIP64_SIZE_AT).ok_or_else(invalid)?,
        ))
    }

    pub fn entries(&mut self) -> Result<Vec<Entry>, String> {
        let (start, size) = self.directory_span()?;
        let directory = self.read_at(start, narrow(size)?)?;
        let mut entries = Vec::new();
        let mut at = 0;
        while u32_at(&directory, at) == Some(CENTRAL_SIGNATURE) {
            let (entry, length) = central_entry(&directory, at).ok_or_else(invalid)?;
            entries.push(entry);
            at = at.saturating_add(length);
        }
        Ok(entries)
    }

    pub fn read(&mut self, entry: Entry) -> Result<Vec<u8>, String> {
        let header = self.read_at(entry.offset, LOCAL_LENGTH)?;
        if u32_at(&header, 0) != Some(LOCAL_SIGNATURE) {
            return Err(invalid());
        }
        let name = u16_at(&header, LOCAL_NAME_AT).ok_or_else(invalid)?;
        let extra = u16_at(&header, LOCAL_EXTRA_AT).ok_or_else(invalid)?;
        let data_offset = entry
            .offset
            .saturating_add(wide(LOCAL_LENGTH))
            .saturating_add(u64::from(name))
            .saturating_add(u64::from(extra));
        let data = self.read_at(data_offset, narrow(entry.size)?)?;
        match entry.method {
            STORED => Ok(data),
            DEFLATED => miniz_oxide::inflate::decompress_to_vec(&data).map_err(|error| error.to_string()),
            _ => Err(invalid()),
        }
    }
}

fn central_entry(directory: &[u8], at: usize) -> Option<(Entry, usize)> {
    let field16 = |offset: usize| u16_at(directory, at.checked_add(offset)?);
    let field32 = |offset: usize| u32_at(directory, at.checked_add(offset)?);
    let method = field16(CENTRAL_METHOD_AT)?;
    let compressed = field32(CENTRAL_COMPRESSED_AT)?;
    let uncompressed = field32(CENTRAL_UNCOMPRESSED_AT)?;
    let name_length = usize::from(field16(CENTRAL_NAME_AT)?);
    let extra_length = usize::from(field16(CENTRAL_EXTRA_AT)?);
    let comment_length = usize::from(field16(CENTRAL_COMMENT_AT)?);
    let local_offset = field32(CENTRAL_OFFSET_AT)?;
    let extra_start = at.checked_add(CENTRAL_LENGTH)?.checked_add(name_length)?;
    let extra = directory.get(extra_start..extra_start.checked_add(extra_length)?)?;
    let mut wide_fields = zip64_fields(extra).into_iter();
    if uncompressed == SATURATED_32 {
        wide_fields.next();
    }
    let size = if compressed == SATURATED_32 {
        wide_fields.next()?
    } else {
        u64::from(compressed)
    };
    let offset = if local_offset == SATURATED_32 {
        wide_fields.next()?
    } else {
        u64::from(local_offset)
    };
    let length = CENTRAL_LENGTH
        .checked_add(name_length)?
        .checked_add(extra_length)?
        .checked_add(comment_length)?;
    Some((Entry { offset, size, method }, length))
}

fn zip64_fields(extra: &[u8]) -> Vec<u64> {
    let mut at = 0;
    while let (Some(id), Some(length)) = (
        u16_at(extra, at),
        u16_at(extra, at.saturating_add(EXTRA_LENGTH_AT)),
    ) {
        let body_start = at.saturating_add(EXTRA_HEADER_LENGTH);
        let body_end = body_start.saturating_add(usize::from(length));
        if id == ZIP64_EXTRA_ID {
            let body = extra.get(body_start..body_end).unwrap_or_default();
            let (chunks, _) = body.as_chunks::<{ size_of::<u64>() }>();
            return chunks.iter().map(|chunk| u64::from_le_bytes(*chunk)).collect();
        }
        at = body_end;
    }
    Vec::new()
}
