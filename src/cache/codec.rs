const CONTINUATION_BIT: u8 = 0x80;
const PAYLOAD_BITS: u8 = 0x7f;
const PAYLOAD_WIDTH: u32 = 7;
const ABSENT: u64 = 0;
const PRESENT: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Corrupt;

#[derive(Debug, Default)]
pub struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn number(&mut self, value: u64) {
        let mut rest = value;
        loop {
            let low = u8::try_from(rest & u64::from(PAYLOAD_BITS)).unwrap_or_default();
            rest >>= PAYLOAD_WIDTH;
            if rest == 0 {
                self.bytes.push(low);
                return;
            }
            self.bytes.push(low | CONTINUATION_BIT);
        }
    }

    pub fn size(&mut self, value: usize) {
        self.number(u64::try_from(value).unwrap_or(u64::MAX));
    }

    pub fn optional_size(&mut self, value: Option<usize>) {
        match value {
            Some(value) => {
                self.number(PRESENT);
                self.size(value);
            }
            None => self.number(ABSENT),
        }
    }

    pub fn fixed(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn raw(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    pub fn text(&mut self, value: &str) {
        self.size(value.len());
        self.raw(value.as_bytes());
    }

    pub fn list<Item>(&mut self, items: &[Item], mut write: impl FnMut(&mut Self, &Item)) {
        self.size(items.len());
        for item in items {
            write(self, item);
        }
    }
}

#[derive(Debug)]
pub struct Reader<'bytes> {
    bytes: &'bytes [u8],
}

impl<'bytes> Reader<'bytes> {
    pub const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes }
    }

    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn number(&mut self) -> Result<u64, Corrupt> {
        let mut value: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            let (&byte, rest) = self.bytes.split_first().ok_or(Corrupt)?;
            self.bytes = rest;
            let payload = u64::from(byte & PAYLOAD_BITS);
            value |= payload
                .checked_shl(shift)
                .filter(|shifted| shifted >> shift == payload)
                .ok_or(Corrupt)?;
            if byte & CONTINUATION_BIT == 0 {
                return Ok(value);
            }
            shift = shift
                .checked_add(PAYLOAD_WIDTH)
                .filter(|&next| next < u64::BITS)
                .ok_or(Corrupt)?;
        }
    }

    pub fn size(&mut self) -> Result<usize, Corrupt> {
        usize::try_from(self.number()?).map_err(|_| Corrupt)
    }

    pub fn optional_size(&mut self) -> Result<Option<usize>, Corrupt> {
        match self.number()? {
            ABSENT => Ok(None),
            PRESENT => self.size().map(Some),
            _ => Err(Corrupt),
        }
    }

    pub fn fixed(&mut self) -> Result<u64, Corrupt> {
        let bytes = self.take(size_of::<u64>())?;
        bytes.try_into().map(u64::from_le_bytes).map_err(|_| Corrupt)
    }

    pub fn take(&mut self, length: usize) -> Result<&'bytes [u8], Corrupt> {
        let (taken, rest) = self.bytes.split_at_checked(length).ok_or(Corrupt)?;
        self.bytes = rest;
        Ok(taken)
    }

    pub fn text(&mut self) -> Result<String, Corrupt> {
        let length = self.size()?;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Corrupt)
    }

    pub fn list<Item>(
        &mut self,
        mut read: impl FnMut(&mut Self) -> Result<Item, Corrupt>,
    ) -> Result<Vec<Item>, Corrupt> {
        let count = self.size()?;
        let mut items = Vec::with_capacity(count.min(self.bytes.len()));
        for _ in 0..count {
            items.push(read(self)?);
        }
        Ok(items)
    }
}
