use super::java::JavaSummary;
use super::java_nullness::ProjectTypes;
use super::java_resources::{created_place, unclosed_class};
use crate::jvm::library::Library;
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_resources::{ResourceCandidate, ResourceOrigin};

const GETTER_PREFIX: &str = "get";
const GETTERS_THAT_CREATE: &[&str] = &["getClass", "getResourceAsStream"];

// Si las llamadas que se encadenan sobre lo creado devuelven todas su mismo tipo (un builder): lo
// que importa es adónde va la de más afuera.
pub(super) fn keeps_its_type(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    candidate: &ResourceCandidate,
) -> bool {
    let mut extended = match &candidate.origin {
        ResourceOrigin::Call(chain) => (**chain).clone(),
        ResourceOrigin::Construction(class) => Chain {
            root: ChainRoot::Instance(class.clone()),
            steps: Vec::new(),
            dereferenced_at: None,
            passed_as: None,
            returned: None,
            nullable_root_arguments: Vec::new(),
            enclosing: Vec::new(),
            forwarded: Vec::new(),
            exact_root: None,
        },
    };
    let Some(created) = created_place(types, library, summary, &extended) else {
        return false;
    };
    candidate.consumers.iter().all(|(name, arity)| {
        extended.steps.push(ChainStep {
            name: name.clone(),
            arity: *arity,
            line: candidate.line,
            column: candidate.column,
            checked: false,
            class_arguments: Vec::new(),
            text_arguments: Vec::new(),
            argument_types: Vec::new(),
            passed_before: Vec::new(),
            dereferenced: false,
        });
        created_place(types, library, summary, &extended).is_some_and(|place| place == created)
    })
}

// Si el receptor crea un recurso (`ResourceInspection.resourcePotentiallyCreatedFromOther` pregunta
// `isResourceCreation`): se cierra, no es de un tipo ignorado y no sale de un getter.
pub(super) fn creates_a_resource(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    receiver: &ResourceOrigin,
) -> bool {
    if let ResourceOrigin::Call(chain) = receiver
        && chain.steps.last().is_some_and(|step| is_getter(&step.name))
    {
        return false;
    }
    unclosed_class(types, library, summary, receiver).is_some()
}

fn is_getter(name: &str) -> bool {
    name.starts_with(GETTER_PREFIX) && !GETTERS_THAT_CREATE.contains(&name)
}
