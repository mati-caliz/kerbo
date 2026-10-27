use super::super::codec::{Corrupt, Reader, Writer};
use super::{read_go_package, write_go_package};
use crate::lint::go::{CapitalizedError, GoPackageFacts, MessagePiece, PendingMessage};

#[test]
fn go_package_facts_survive_the_round_trip() {
    let facts = GoPackageFacts {
        package: "store".to_owned(),
        declared: vec!["Load".to_owned()],
        capitalized: vec![CapitalizedError {
            line: 3,
            column: 9,
            word: "Missing".to_owned(),
        }],
        outside_functions: vec![(1, 2)],
        constants: vec![("errLoad".to_owned(), "Load failed".to_owned())],
        pending: vec![PendingMessage {
            line: 7,
            column: 10,
            callee: "fmt.Errorf".to_owned(),
            pieces: vec![
                MessagePiece::Constant("errLoad".to_owned()),
                MessagePiece::Text(" %q.".to_owned()),
            ],
        }],
    };
    let mut writer = Writer::default();
    write_go_package(&mut writer, Some(&facts));
    write_go_package(&mut writer, None);
    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    assert_eq!(read_go_package(&mut reader), Ok(Some(facts)));
    assert_eq!(read_go_package(&mut reader), Ok(None));
    assert_eq!(read_go_package(&mut Reader::new(&[2])), Err(Corrupt));
}
