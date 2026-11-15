use super::codec::{Corrupt, Reader, Writer};
use super::summary_codec::{read_position, write_position};
use super::throws_codec::{read_origin, write_origin};
use crate::index::java::JavaSummary;
use crate::index::java_annotation_values::{AnnotationDefault, AnnotationValue};
use crate::lint::java_import_shadows::{Enclosing, ImportShadow};
use crate::lint::java_sequenced_collection::SequencedCandidate;
use crate::lint::java_size_is_empty::SizeCandidate;

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

// Lo que el índice resuelve con los tipos del proyecto y de las dependencias: las listas de
// `getFirst()`, los imports que tapa un tipo heredado y los `size()` sobre una cadena.
pub(super) fn write_resolution_candidates(writer: &mut Writer, summary: &JavaSummary) {
    writer.list(&summary.sequenced_candidates, write_sequenced_candidate);
    writer.list(&summary.nested_types, |writer, (outer, inner)| {
        writer.text(outer);
        writer.text(inner);
    });
    writer.list(&summary.import_shadows, write_import_shadow);
    writer.list(&summary.size_candidates, |writer, candidate| {
        write_origin(writer, &candidate.origin);
        writer.text(&candidate.replacement);
        writer.size(candidate.line);
        writer.size(candidate.column);
    });
    writer.list(&summary.array_methods, |writer, name| writer.text(name));
}

pub(super) fn read_resolution_candidates(
    reader: &mut Reader<'_>,
    summary: &mut JavaSummary,
) -> Result<(), Corrupt> {
    summary.sequenced_candidates = reader.list(read_sequenced_candidate)?;
    summary.nested_types = reader.list(|reader| Ok((reader.text()?, reader.text()?)))?;
    summary.import_shadows = reader.list(read_import_shadow)?;
    summary.size_candidates = reader.list(|reader| {
        Ok(SizeCandidate {
            origin: read_origin(reader)?,
            replacement: reader.text()?,
            line: reader.size()?,
            column: reader.size()?,
        })
    })?;
    summary.array_methods = reader.list(Reader::text)?;
    Ok(())
}

fn write_sequenced_candidate(writer: &mut Writer, candidate: &SequencedCandidate) {
    writer.text(&candidate.type_name);
    writer.number(u64::from(candidate.needs_list));
    writer.text(&candidate.method);
    writer.size(candidate.line);
    writer.size(candidate.column);
}

fn read_sequenced_candidate(reader: &mut Reader<'_>) -> Result<SequencedCandidate, Corrupt> {
    Ok(SequencedCandidate {
        type_name: reader.text()?,
        needs_list: reader.number()? != 0,
        method: reader.text()?,
        line: reader.size()?,
        column: reader.size()?,
    })
}

fn write_import_shadow(writer: &mut Writer, shadow: &ImportShadow) {
    writer.text(&shadow.imported);
    writer.size(shadow.line);
    writer.size(shadow.column);
    writer.list(&shadow.usages, |writer, usage| {
        writer.list(usage, |writer, enclosing| {
            writer.text(enclosing.name.as_deref().unwrap_or_default());
            writer.list(&enclosing.supertypes, |writer, supertype| writer.text(supertype));
        });
    });
}

fn read_import_shadow(reader: &mut Reader<'_>) -> Result<ImportShadow, Corrupt> {
    Ok(ImportShadow {
        imported: reader.text()?,
        line: reader.size()?,
        column: reader.size()?,
        usages: reader.list(|reader| {
            reader.list(|reader| {
                let name = reader.text()?;
                Ok(Enclosing {
                    name: (!name.is_empty()).then_some(name),
                    supertypes: reader.list(Reader::text)?,
                })
            })
        })?,
    })
}

#[cfg(test)]
mod tests;
