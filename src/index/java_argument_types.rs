use super::java::JavaSummary;
use super::java_chain_walk::chain_class;
use super::java_library_arguments::{called_type, library_inferred_non_null};
use super::java_nullness::{ProjectTypes, Resolved, resolve};
use crate::jvm::library::Library;
use crate::lint::java_chains::Chain;
use crate::lint::java_null_flow::NullOrigin;
use crate::lint::java_null_inference::NullArgument;
use crate::lint::java_receivers::Receiver;

// El parámetro de librería que el bytecode exige no nulo, eligiendo la sobrecarga por el tipo de lo
// que se le pasa: lo que devuelve la cadena (`p.stdout()` es un `OutputStream`, así que
// `new PrintStream(p.stdout())` no es `PrintStream(String)`). Sólo si ese tipo es de librería y se
// puede leer; si no, no se sabe cuál corre.
pub(super) fn typed_library_parameter(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (argument, value): (&NullArgument, &Chain),
) -> Option<String> {
    let Resolved::Library(passed) = chain_class(types, library, summary, value)? else {
        return None;
    };
    library.class_or_jdk(&passed)?;
    let class = receiver_library_class(types, library, summary, argument)?;
    library_inferred_non_null(library, &class, argument, &library.supertypes(&passed))
}

pub(super) fn receiver_library_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    argument: &NullArgument,
) -> Option<String> {
    let written = match &argument.receiver {
        Receiver::Type(written) => written.clone(),
        Receiver::Unknown => called_type(
            types,
            library,
            summary,
            argument.receiver_arguments.called.as_deref()?,
        )?,
        Receiver::Scope(_) => return None,
    };
    match resolve(types, library, summary, &written) {
        Resolved::Library(class) => Some(class),
        Resolved::Project(_) => None,
    }
}

// La llamada de la que salió el valor de una variable, si salió de una.
pub(super) fn origin_chain(origin: &NullOrigin) -> Option<&Chain> {
    match origin {
        NullOrigin::Call(chain) => Some(chain),
        NullOrigin::Literal | NullOrigin::Annotated | NullOrigin::QualifiedField { .. } => None,
    }
}
