use super::codec::{Corrupt, Reader, Writer};
use super::summary_codec::{read_position, write_position};
use crate::index::java_annotation_values::{AnnotationDefault, AnnotationValue};
use crate::lint::java_sequenced_collection::SequencedCandidate;

pub(super) fn write_annotation_default(writer: &mut Writer, declared: &AnnotationDefault) {
    writer.text(&declared.annotation);
    writer.text(&declared.element);
    writer.text(&declared.value);
}

pub(super) fn read_annotation_default(reader: &mut Reader<'_>) -> Result<AnnotationDefault, Corrupt> {
    Ok(AnnotationDefault {
        annotation: reader.text()?,
        element: reader.text()?,
        value: reader.text()?,
    })
}

pub(super) fn write_annotation_value(writer: &mut Writer, written: &AnnotationValue) {
    writer.text(&written.written);
    writer.list(&written.enclosing, |writer, name| writer.text(name));
    writer.text(&written.element);
    writer.text(&written.value);
    write_position(writer, &written.position);
}

pub(super) fn read_annotation_value(reader: &mut Reader<'_>) -> Result<AnnotationValue, Corrupt> {
    Ok(AnnotationValue {
        written: reader.text()?,
        enclosing: reader.list(Reader::text)?,
        element: reader.text()?,
        value: reader.text()?,
        position: read_position(reader)?,
    })
}

pub(super) fn write_sequenced_candidate(writer: &mut Writer, candidate: &SequencedCandidate) {
    writer.text(&candidate.type_name);
    writer.number(u64::from(candidate.needs_list));
    writer.text(&candidate.method);
    writer.size(candidate.line);
    writer.size(candidate.column);
}

pub(super) fn read_sequenced_candidate(reader: &mut Reader<'_>) -> Result<SequencedCandidate, Corrupt> {
    Ok(SequencedCandidate {
        type_name: reader.text()?,
        needs_list: reader.number()? != 0,
        method: reader.text()?,
        line: reader.size()?,
        column: reader.size()?,
    })
}

#[cfg(test)]
mod tests;
