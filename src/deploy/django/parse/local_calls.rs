use tree_sitter::Node;

use super::{Scope, argument};
use crate::deploy::django::constants::Constants;
use crate::deploy::django::factories::{parameter_names, returned_call};
use crate::deploy::django::operations::Operation;
use crate::deploy::python_calls::Call;

pub(super) fn module_function_operation<'context>(
    scope: Scope<'context>,
    call: &Call<'context>,
) -> Option<Operation> {
    with_module_function(scope, call, |inner, returned| {
        inner.declared_operation(&Call::parse(returned, inner.source)?)
    })
}

pub(super) fn with_module_function<'context, T>(
    scope: Scope<'context>,
    call: &Call<'context>,
    read: impl for<'bound> FnOnce(Scope<'bound>, Node<'bound>) -> Option<T>,
) -> Option<T> {
    if !call.receiver.is_empty() {
        return None;
    }
    let function = scope
        .functions
        .iter()
        .rev()
        .find(|(name, _)| name == call.method)
        .map(|(_, function)| *function)?;
    let parameters = parameter_names(function.child_by_field_name("parameters")?, scope.source);
    let returned = returned_call(
        function.child_by_field_name("body")?,
        scope.source,
        scope.settings,
    )?;
    let mut bindings = scope.constants.bindings.to_vec();
    bindings.extend(parameters.iter().enumerate().filter_map(|(position, name)| {
        Some((name.clone(), scope.resolved(argument(call, position, name)?)))
    }));
    let scope = Scope {
        constants: Constants::new(scope.source, &bindings),
        ..scope
    };
    read(scope, returned)
}

pub(super) fn factory_operation<'context>(
    scope: Scope<'context>,
    call: &Call<'context>,
) -> Option<Operation> {
    if !call.receiver.is_empty() {
        return None;
    }
    let factory = scope
        .factories
        .iter()
        .rev()
        .find(|(name, _)| name == call.method)
        .map(|(_, factory)| factory)?;
    let arguments = factory
        .forwarded
        .iter()
        .map(|parameter| argument(call, parameter.position, &parameter.name))
        .collect::<Option<Vec<_>>>()?;
    scope.declared_operation(&Call {
        receiver: "",
        method: &factory.method,
        line: call.line,
        column: call.column,
        arguments,
        source: call.source,
    })
}
