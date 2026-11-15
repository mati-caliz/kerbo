use super::super::codec::{Reader, Writer};
use super::{
    read_annotation_default, read_annotation_value, write_annotation_default, write_annotation_value,
};
use crate::index::java::Position;
use crate::index::java_annotation_values::{AnnotationDefault, AnnotationValue};

#[test]
fn a_declared_default_survives_the_round_trip() {
    let declared = AnnotationDefault {
        annotation: "org.junit.Test".to_owned(),
        element: "timeout".to_owned(),
        value: "0L".to_owned(),
    };
    let mut writer = Writer::default();
    write_annotation_default(&mut writer, &declared);
    let bytes = writer.into_bytes();
    assert_eq!(read_annotation_default(&mut Reader::new(&bytes)), Ok(declared));
}

#[test]
fn a_written_value_survives_the_round_trip() {
    let written = AnnotationValue {
        written: "Test".to_owned(),
        enclosing: vec!["Outer".to_owned(), "Inner".to_owned()],
        element: "timeout".to_owned(),
        value: "0L".to_owned(),
        position: Position { line: 12, column: 11 },
    };
    let mut writer = Writer::default();
    write_annotation_value(&mut writer, &written);
    let bytes = writer.into_bytes();
    assert_eq!(read_annotation_value(&mut Reader::new(&bytes)), Ok(written));
}

#[test]
fn what_the_index_resolves_with_the_dependencies_survives_the_round_trip() {
    use super::{read_resolution_candidates, write_resolution_candidates};
    use crate::index::java::JavaSummary;
    use crate::lint::java_import_shadows::{Enclosing, ImportShadow};
    use crate::lint::java_sequenced_collection::SequencedCandidate;
    use crate::lint::java_size_is_empty::SizeCandidate;
    use crate::lint::java_throws::{CallRoot, CallStep, ThrowOrigin};
    let summary = JavaSummary {
        sequenced_candidates: vec![SequencedCandidate {
            type_name: "DomNodeList".to_owned(),
            needs_list: true,
            method: "getFirst".to_owned(),
            line: 5,
            column: 9,
        }],
        nested_types: vec![("Bag".to_owned(), "Item".to_owned())],
        import_shadows: vec![ImportShadow {
            imported: "java.util.Map.Entry".to_owned(),
            line: 3,
            column: 8,
            usages: vec![vec![
                Enclosing {
                    name: Some("Shelf".to_owned()),
                    supertypes: vec!["HashMap".to_owned()],
                },
                Enclosing {
                    name: None,
                    supertypes: Vec::new(),
                },
            ]],
        }],
        size_candidates: vec![SizeCandidate {
            origin: ThrowOrigin::Call(
                CallRoot::Type("Result".to_owned()),
                vec![CallStep {
                    name: "getProblems".to_owned(),
                    arity: 0,
                    arguments: Vec::new(),
                }],
            ),
            replacement: "!result.getProblems().isEmpty()".to_owned(),
            line: 7,
            column: 16,
        }],
        array_methods: vec!["getNames".to_owned()],
        ..JavaSummary::default()
    };
    let mut writer = Writer::default();
    write_resolution_candidates(&mut writer, &summary);
    let bytes = writer.into_bytes();
    let mut read = JavaSummary::default();
    assert_eq!(
        read_resolution_candidates(&mut Reader::new(&bytes), &mut read),
        Ok(())
    );
    assert_eq!(read, summary);
}
