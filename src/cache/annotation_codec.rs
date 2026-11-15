use super::codec::{Corrupt, Reader, Writer};
use super::summary_codec::{read_position, write_position};
use crate::index::java_annotation_values::{AnnotationDefault, AnnotationValue};

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

#[cfg(test)]
mod tests;
