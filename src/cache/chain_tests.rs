use super::chain_codec::{read_flow, write_flow};
use super::codec::{Reader, Writer};
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_null_flow::{Dereference, NullFlow};

#[test]
fn chains_and_null_flows_survive_the_round_trip() {
    let flow = NullFlow {
        origin: Chain {
            root: ChainRoot::Instance("JdbcTemplate".to_owned()),
            steps: vec![ChainStep {
                name: "queryForObject".to_owned(),
                arity: 3,
                line: 12,
                column: 21,
                checked: false,
                class_arguments: vec![1],
            }],
            unboxed: Some((12, 16)),
        },
        dereferences: vec![Dereference {
            name: "intValue".to_owned(),
            line: 13,
            column: 9,
        }],
    };
    let classes = NullFlow {
        origin: Chain {
            root: ChainRoot::Class("System".to_owned()),
            unboxed: None,
            ..flow.origin.clone()
        },
        dereferences: Vec::new(),
    };
    let mut writer = Writer::default();
    write_flow(&mut writer, &flow);
    write_flow(&mut writer, &classes);
    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    assert_eq!(read_flow(&mut reader), Ok(flow));
    assert_eq!(read_flow(&mut reader), Ok(classes));
    assert!(read_flow(&mut Reader::new(&[9])).is_err());
}
