struct Signature {
    offset: usize,
    magic: &'static [u8],
}

const fn at_start(magic: &'static [u8]) -> Signature {
    Signature { offset: 0, magic }
}

const TAR_OFFSET: usize = 257;
const DICOM_OFFSET: usize = 128;

const APPLICATION_SIGNATURES: &[Signature] = &[
    at_start(b"PK\x03\x04"),
    at_start(b"PK\x05\x06"),
    at_start(b"PK\x07\x08"),
    Signature {
        offset: TAR_OFFSET,
        magic: b"ustar",
    },
    Signature {
        offset: DICOM_OFFSET,
        magic: b"DICM",
    },
    at_start(b"Rar!\x1a\x07"),
    at_start(b"\x1f\x8b\x08"),
    at_start(b"BZh"),
    at_start(b"7z\xbc\xaf\x27\x1c"),
    at_start(b"\xfd7zXZ\x00"),
    at_start(b"%PDF"),
    at_start(b"MZ"),
    at_start(b"CWS"),
    at_start(b"FWS"),
    at_start(b"{\\rtf"),
    at_start(b"%!"),
    at_start(b"SQLi"),
    at_start(b"NES\x1a"),
    at_start(b"Cr24"),
    at_start(b"MSCF"),
    at_start(b"ISc("),
    at_start(b"!<arch>"),
    at_start(b"\x1f\xa0"),
    at_start(b"\x1f\x9d"),
    at_start(b"LZIP"),
    at_start(b"\xed\xab\xee\xdb"),
    at_start(b"\x7fELF"),
    at_start(b"\xfe\xed\xfa\xce"),
    at_start(b"\xce\xfa\xed\xfe"),
    at_start(b"\xfe\xed\xfa\xcf"),
    at_start(b"\xcf\xfa\xed\xfe"),
    at_start(b"\xca\xfe\xba\xbe"),
    at_start(b"\x28\xb5\x2f\xfd"),
    at_start(b"\x00asm"),
    at_start(b"dex\n"),
    at_start(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"),
    at_start(b"wOFF"),
    at_start(b"wOF2"),
    at_start(b"\x00\x01\x00\x00\x00"),
    at_start(b"OTTO"),
];

pub fn is_binary(content: &[u8]) -> bool {
    APPLICATION_SIGNATURES.iter().any(|signature| {
        content
            .get(signature.offset..)
            .is_some_and(|rest| rest.starts_with(signature.magic))
    })
}
