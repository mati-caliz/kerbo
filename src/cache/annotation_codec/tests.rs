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
