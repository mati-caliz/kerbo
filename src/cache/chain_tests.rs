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
                text_arguments: vec!["java.version".to_owned()],
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

#[test]
fn what_a_method_can_throw_survives_the_round_trip() {
    use super::throws_codec::{read_method_throws, write_method_throws};
    use crate::lint::java_throws::{
        ArgumentKind, CallRoot, CallStep, MethodThrows, ThrowOrigin, ThrowPoint, ThrowsClause,
    };
    let step = CallStep {
        name: "leer".to_owned(),
        arity: 2,
        arguments: vec![
            ArgumentKind::Text,
            ArgumentKind::Charset,
            ArgumentKind::Lambda(vec![ThrowPoint {
                origin: ThrowOrigin::Thrown("SQLException".to_owned()),
                caught: vec!["Exception".to_owned()],
            }]),
            ArgumentKind::Reference(CallRoot::Type("Archivos".to_owned()), "leer".to_owned()),
        ],
    };
    let origins = [
        ThrowOrigin::Unknown,
        ThrowOrigin::Thrown("IOException".to_owned()),
        ThrowOrigin::Creation("FileInputStream".to_owned(), 1),
        ThrowOrigin::Close("InputStream".to_owned()),
        ThrowOrigin::Call(CallRoot::Scope(vec!["Servicio".to_owned()]), vec![step.clone()]),
        ThrowOrigin::Call(CallRoot::Type("Repositorio".to_owned()), vec![step]),
    ];
    let method = MethodThrows {
        line: 3,
        column: 9,
        clauses: vec![ThrowsClause {
            written: "Exception".to_owned(),
            line: 3,
            column: 30,
        }],
        points: origins
            .into_iter()
            .map(|origin| ThrowPoint {
                origin,
                caught: vec!["IOException".to_owned()],
            })
            .collect(),
    };
    let mut writer = Writer::default();
    write_method_throws(&mut writer, &method);
    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    assert_eq!(read_method_throws(&mut reader), Ok(method));
}

#[test]
fn null_arguments_survive_the_round_trip() {
    use super::null_codec::{read_null_argument, write_null_argument};
    use crate::lint::java_null_inference::NullArgument;
    use crate::lint::java_receivers::Receiver;

    let argument = NullArgument {
        name: "concat".to_owned(),
        is_constructor: false,
        arity: 1,
        receiver: Receiver::Type("ArrayUtils".to_owned()),
        index: 0,
        line: 35,
        column: 88,
    };
    let mut writer = Writer::default();
    write_null_argument(&mut writer, &argument);
    let bytes = writer.into_bytes();
    assert_eq!(read_null_argument(&mut Reader::new(&bytes)), Ok(argument));
}
