use super::chain_codec::{read_flow, write_flow};
use super::codec::{Reader, Writer};
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_null_flow::{Dereference, NullFlow, NullOrigin, NullReturn};
use crate::lint::java_null_inference::NullArgument;
use crate::lint::java_owner::NullMarking;
use crate::lint::java_receiver_arguments::{ArgumentNullness, ReceiverArguments};
use crate::lint::java_receivers::Receiver;

#[test]
fn chains_and_null_flows_survive_the_round_trip() {
    let flow = NullFlow {
        origin: NullOrigin::Call(Chain {
            root: ChainRoot::Instance("JdbcTemplate".to_owned()),
            steps: vec![ChainStep {
                name: "queryForObject".to_owned(),
                arity: 3,
                line: 12,
                column: 21,
                checked: false,
                class_arguments: vec![1],
                text_arguments: vec!["java.version".to_owned()],
                argument_types: vec![Some("String".to_owned()), None],
                passed_before: Vec::new(),
            }],
            unboxed: Some((12, 16)),
            passed_as: None,
            returned: None,
        }),
        dereferences: vec![Dereference {
            name: "intValue".to_owned(),
            line: 13,
            column: 9,
        }],
        arguments: vec![NullArgument {
            name: "unroll".to_owned(),
            is_constructor: false,
            arity: 2,
            receiver: Receiver::Scope(vec!["TypeUtils".to_owned()]),
            index: 1,
            line: 14,
            column: 30,
            cast: None,
            exact: false,
            receiver_arguments: ReceiverArguments::default(),
        }],
        passed_before: Vec::new(),
        returns: Vec::new(),
    };
    let NullOrigin::Call(called) = &flow.origin else {
        unreachable!("el flujo de arriba sale de una llamada");
    };
    let classes = NullFlow {
        origin: NullOrigin::Call(Chain {
            root: ChainRoot::Class("System".to_owned()),
            steps: called
                .steps
                .iter()
                .map(|step| ChainStep {
                    passed_before: flow.arguments.clone(),
                    ..step.clone()
                })
                .collect(),
            unboxed: None,
            passed_as: flow.arguments.first().cloned().map(Box::new),
            returned: None,
        }),
        passed_before: flow.arguments.clone(),
        ..NullFlow::new(NullOrigin::Literal)
    };
    let field = NullFlow::new(NullOrigin::NullableField);
    let mut writer = Writer::default();
    write_flow(&mut writer, &flow);
    write_flow(&mut writer, &classes);
    write_flow(&mut writer, &field);
    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    assert_eq!(read_flow(&mut reader), Ok(flow));
    assert_eq!(read_flow(&mut reader), Ok(classes));
    assert_eq!(read_flow(&mut reader), Ok(field));
    assert!(read_flow(&mut Reader::new(&[9])).is_err());
}

#[test]
fn null_returns_survive_the_round_trip() {
    let returned = |name: &str, marking| NullReturn {
        name: name.to_owned(),
        line: 20,
        column: 16,
        marking,
    };
    let literal = NullFlow {
        returns: vec![returned("null", NullMarking::Marked)],
        ..NullFlow::new(NullOrigin::Literal)
    };
    let call = NullFlow::new(NullOrigin::Call(Chain {
        root: ChainRoot::Class("Converter".to_owned()),
        steps: Vec::new(),
        unboxed: None,
        passed_as: None,
        returned: Some(returned("convert(a)", NullMarking::Unspecified)),
    }));
    let mut writer = Writer::default();
    write_flow(&mut writer, &literal);
    write_flow(&mut writer, &call);
    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    assert_eq!(read_flow(&mut reader), Ok(literal));
    assert_eq!(read_flow(&mut reader), Ok(call));
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

    let argument = NullArgument {
        name: "concat".to_owned(),
        is_constructor: false,
        arity: 1,
        receiver: Receiver::Type("ArrayUtils".to_owned()),
        index: 0,
        line: 35,
        column: 88,
        cast: Some("boolean[][]".to_owned()),
        exact: false,
        receiver_arguments: ReceiverArguments {
            marking: NullMarking::Marked,
            arguments: vec![
                ArgumentNullness::Concrete,
                ArgumentNullness::Nullable,
                ArgumentNullness::Unknown,
            ],
        },
    };
    let mut writer = Writer::default();
    write_null_argument(&mut writer, &argument);
    let bytes = writer.into_bytes();
    assert_eq!(read_null_argument(&mut Reader::new(&bytes)), Ok(argument));
}

#[test]
fn contract_calls_survive_the_round_trip() {
    use super::null_codec::{read_contract_call, write_contract_call};
    use crate::lint::java_contract_calls::{CallTarget, ContractCall};
    use crate::lint::java_contracts::Constraint;

    let call = ContractCall {
        name: "isTrue".to_owned(),
        target: CallTarget::ImportedMethod,
        arity: 2,
        receiver: Receiver::Scope(vec!["ValidateTest".to_owned()]),
        arguments: vec![Some(Constraint::False), None, Some(Constraint::NotNull)],
        line: 12,
        column: 18,
        exception_expected: true,
    };
    let mut writer = Writer::default();
    write_contract_call(&mut writer, &call);
    let bytes = writer.into_bytes();
    assert_eq!(read_contract_call(&mut Reader::new(&bytes)), Ok(call));
}

#[test]
fn lambdas_returning_null_survive_the_round_trip() {
    use super::lambda_codec::{read_lambda_return, write_lambda_return};
    use crate::lint::java_lambda_returns::{LambdaReturn, LambdaTarget};
    use crate::lint::java_type_arguments::TypeArgument;

    let returned = NullReturn {
        name: "null".to_owned(),
        line: 73,
        column: 31,
        marking: NullMarking::Marked,
    };
    let lambdas = [
        LambdaReturn {
            returns: vec![returned.clone()],
            target: LambdaTarget::Argument {
                argument: NullArgument {
                    name: "toImmutableTable".to_owned(),
                    is_constructor: false,
                    arity: 3,
                    receiver: Receiver::Type("TableCollectors".to_owned()),
                    index: 0,
                    line: 73,
                    column: 26,
                    cast: None,
                    exact: false,
                    receiver_arguments: ReceiverArguments::default(),
                },
                target_typed: true,
            },
        },
        LambdaReturn {
            returns: vec![returned],
            target: LambdaTarget::Declared {
                type_name: "BiFunction".to_owned(),
                arguments: vec![
                    TypeArgument::Concrete,
                    TypeArgument::Nullable,
                    TypeArgument::MethodVariable,
                    TypeArgument::OtherVariable,
                    TypeArgument::Wildcard,
                ],
            },
        },
    ];
    for lambda in lambdas {
        let mut writer = Writer::default();
        write_lambda_return(&mut writer, &lambda);
        let bytes = writer.into_bytes();
        assert_eq!(read_lambda_return(&mut Reader::new(&bytes)), Ok(lambda));
    }
}
