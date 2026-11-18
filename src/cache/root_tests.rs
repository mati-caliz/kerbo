use super::chain_codec::{read_flow, write_flow};
use super::codec::{Reader, Writer};
use crate::lint::java_chains::{Chain, ChainRoot};
use crate::lint::java_null_flow::{NullFlow, NullOrigin};
use crate::lint::java_null_inference::{ArgumentShape, NullArgument};
use crate::lint::java_owner::NullMarking;
use crate::lint::java_receiver_arguments::ReceiverArguments;
use crate::lint::java_receivers::Receiver;

fn chain(root: ChainRoot) -> Chain {
    Chain {
        root,
        steps: Vec::new(),
        dereferenced_at: None,
        passed_as: None,
        returned: None,
        nullable_root_arguments: Vec::new(),
        enclosing: Vec::new(),
        forwarded: Vec::new(),
        exact_root: None,
    }
}

// Las raíces sin tipo escrito llevan otra raíz adentro: el `var` la cadena que lo inicializa, y los
// campos leídos desde el parámetro de una lambda, la llamada que recibe esa lambda.
#[test]
fn inferred_chain_roots_survive_the_round_trip() {
    let slot = NullArgument {
        name: "then".to_owned(),
        is_constructor: false,
        arity: 1,
        receiver: Receiver::Type("JenkinsSessionRule".to_owned()),
        index: 0,
        line: 52,
        column: 23,
        cast: None,
        exact: false,
        shape: ArgumentShape::MaybeArray,
        receiver_arguments: ReceiverArguments {
            marking: NullMarking::Unspecified,
            arguments: Vec::new(),
            inherited: None,
            called: None,
        },
    };
    let lambda_field = ChainRoot::Field {
        of: Box::new(ChainRoot::LambdaParameter {
            slot: Box::new(slot),
            position: 0,
        }),
        fields: vec!["jenkins".to_owned()],
    };
    let initialized = ChainRoot::Returned(Box::new(chain(lambda_field)));
    let flow = NullFlow::new(NullOrigin::Call(Box::new(chain(initialized))));
    let mut writer = Writer::default();
    write_flow(&mut writer, &flow);
    let bytes = writer.into_bytes();
    assert_eq!(read_flow(&mut Reader::new(&bytes)), Ok(flow));
}
