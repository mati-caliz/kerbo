use super::super::codec::{Reader, Writer};
use super::{read_generics, write_generics};
use crate::lint::java_generics::{Carrier, GenericParent, Generics, MethodGenerics, TypeGenerics};

#[test]
fn the_generics_of_a_file_survive_the_round_trip() {
    let generics = Generics {
        types: vec![TypeGenerics {
            name: "Opening".to_owned(),
            parameters: vec!["T".to_owned()],
            parents: vec![GenericParent {
                written: "Source".to_owned(),
                arguments: vec!["T".to_owned(), "IOException".to_owned()],
            }],
        }],
        methods: vec![MethodGenerics {
            line: 4,
            column: 32,
            parameters: vec!["E".to_owned()],
            carriers: vec![Carrier {
                parameter: "E".to_owned(),
                position: 0,
                interface: "FailableConsumer".to_owned(),
                argument: 1,
            }],
        }],
    };
    let mut writer = Writer::default();
    write_generics(&mut writer, &generics);
    let bytes = writer.into_bytes();
    assert_eq!(read_generics(&mut Reader::new(&bytes)), Ok(generics));
}
