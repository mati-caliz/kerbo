use super::codec::{Corrupt, Reader, Writer};
use crate::secrets::SecretFinding;

pub fn encode(findings: &[SecretFinding]) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.list(findings, |writer, finding| {
        writer.text(&finding.kind);
        writer.size(finding.line);
        writer.size(finding.column);
        writer.size(finding.end_line);
        writer.text(&finding.preview);
        writer.size(finding.length);
    });
    writer.into_bytes()
}

pub fn decode(bytes: &[u8]) -> Result<Vec<SecretFinding>, Corrupt> {
    let mut reader = Reader::new(bytes);
    let findings = reader.list(|reader| {
        Ok(SecretFinding {
            kind: reader.text()?,
            line: reader.size()?,
            column: reader.size()?,
            end_line: reader.size()?,
            preview: reader.text()?,
            length: reader.size()?,
        })
    })?;
    if reader.is_empty() {
        Ok(findings)
    } else {
        Err(Corrupt)
    }
}
