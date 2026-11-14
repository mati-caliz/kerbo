use super::codec::{Corrupt, Reader, Writer};
use super::declaration_codec::{read_marking, write_marking};
use super::summary_codec::{read_receiver, write_receiver};
use crate::lint::java_called_receivers::CalledReceiver;
use crate::lint::java_contract_calls::{CallTarget, ContractCall};
use crate::lint::java_contracts::Constraint;
use crate::lint::java_nested_arguments::{NestedNull, ReferenceArgument};
use crate::lint::java_null_inference::NullArgument;
use crate::lint::java_receiver_arguments::{ArgumentNullness, InheritedMember, ReceiverArguments};

const CONSTRAINT_ANY: u64 = 0;
const CONSTRAINT_NULL: u64 = 1;
const CONSTRAINT_NOT_NULL: u64 = 2;
const CONSTRAINT_TRUE: u64 = 3;
const CONSTRAINT_FALSE: u64 = 4;
const TARGET_CONSTRUCTOR: u64 = 0;
const TARGET_METHOD: u64 = 1;
const TARGET_IMPORTED_METHOD: u64 = 2;
const ARGUMENT_CONCRETE: u64 = 0;
const ARGUMENT_NULLABLE: u64 = 1;
const ARGUMENT_UNKNOWN: u64 = 2;
const ARGUMENT_NON_NULL_VARIABLE: u64 = 3;

pub fn write_null_argument(writer: &mut Writer, argument: &NullArgument) {
    writer.text(&argument.name);
    writer.number(u64::from(argument.is_constructor));
    writer.size(argument.arity);
    write_receiver(writer, &argument.receiver);
    writer.size(argument.index);
    writer.size(argument.line);
    writer.size(argument.column);
    writer.list(argument.cast.as_slice(), |writer, cast| writer.text(cast));
    writer.number(u64::from(argument.exact));
    write_marking(writer, argument.receiver_arguments.marking);
    writer.list(&argument.receiver_arguments.arguments, |writer, nullness| {
        writer.number(match nullness {
            ArgumentNullness::Concrete => ARGUMENT_CONCRETE,
            ArgumentNullness::Nullable => ARGUMENT_NULLABLE,
            ArgumentNullness::Unknown => ARGUMENT_UNKNOWN,
            ArgumentNullness::NonNullVariable => ARGUMENT_NON_NULL_VARIABLE,
        });
    });
    writer.list(
        argument.receiver_arguments.inherited.as_slice(),
        |writer, member| {
            writer.list(&member.scope, |writer, name| writer.text(name));
            writer.text(&member.name);
            writer.number(u64::from(member.getter));
        },
    );
    writer.list(argument.receiver_arguments.called.as_slice(), |writer, called| {
        writer.text(&called.name);
        writer.size(called.arity);
        write_receiver(writer, &called.receiver);
    });
}

pub fn read_null_argument(reader: &mut Reader<'_>) -> Result<NullArgument, Corrupt> {
    Ok(NullArgument {
        name: reader.text()?,
        is_constructor: reader.number()? != 0,
        arity: reader.size()?,
        receiver: read_receiver(reader)?,
        index: reader.size()?,
        line: reader.size()?,
        column: reader.size()?,
        cast: reader.list(Reader::text)?.into_iter().next(),
        exact: reader.number()? != 0,
        receiver_arguments: ReceiverArguments {
            marking: read_marking(reader)?,
            arguments: reader.list(|reader| match reader.number()? {
                ARGUMENT_CONCRETE => Ok(ArgumentNullness::Concrete),
                ARGUMENT_NULLABLE => Ok(ArgumentNullness::Nullable),
                ARGUMENT_UNKNOWN => Ok(ArgumentNullness::Unknown),
                ARGUMENT_NON_NULL_VARIABLE => Ok(ArgumentNullness::NonNullVariable),
                _ => Err(Corrupt),
            })?,
            inherited: reader
                .list(|reader| {
                    Ok(InheritedMember {
                        scope: reader.list(Reader::text)?,
                        name: reader.text()?,
                        getter: reader.number()? != 0,
                    })
                })?
                .into_iter()
                .next(),
            called: reader
                .list(|reader| {
                    Ok(Box::new(CalledReceiver {
                        name: reader.text()?,
                        arity: reader.size()?,
                        receiver: read_receiver(reader)?,
                    }))
                })?
                .into_iter()
                .next(),
        },
    })
}

pub fn write_constraint(writer: &mut Writer, constraint: Option<&Constraint>) {
    writer.number(match constraint {
        None => CONSTRAINT_ANY,
        Some(Constraint::Null) => CONSTRAINT_NULL,
        Some(Constraint::NotNull) => CONSTRAINT_NOT_NULL,
        Some(Constraint::True) => CONSTRAINT_TRUE,
        Some(Constraint::False) => CONSTRAINT_FALSE,
    });
}

pub fn read_constraint(reader: &mut Reader<'_>) -> Result<Option<Constraint>, Corrupt> {
    match reader.number()? {
        CONSTRAINT_ANY => Ok(None),
        CONSTRAINT_NULL => Ok(Some(Constraint::Null)),
        CONSTRAINT_NOT_NULL => Ok(Some(Constraint::NotNull)),
        CONSTRAINT_TRUE => Ok(Some(Constraint::True)),
        CONSTRAINT_FALSE => Ok(Some(Constraint::False)),
        _ => Err(Corrupt),
    }
}

pub fn write_contract_call(writer: &mut Writer, call: &ContractCall) {
    writer.text(&call.name);
    writer.number(match call.target {
        CallTarget::Constructor => TARGET_CONSTRUCTOR,
        CallTarget::Method => TARGET_METHOD,
        CallTarget::ImportedMethod => TARGET_IMPORTED_METHOD,
    });
    writer.size(call.arity);
    write_receiver(writer, &call.receiver);
    writer.list(&call.arguments, |writer, argument| {
        write_constraint(writer, argument.as_ref());
    });
    writer.size(call.line);
    writer.size(call.column);
    writer.number(u64::from(call.exception_expected));
}

pub fn read_contract_call(reader: &mut Reader<'_>) -> Result<ContractCall, Corrupt> {
    Ok(ContractCall {
        name: reader.text()?,
        target: read_target(reader)?,
        arity: reader.size()?,
        receiver: read_receiver(reader)?,
        arguments: reader.list(read_constraint)?,
        line: reader.size()?,
        column: reader.size()?,
        exception_expected: reader.number()? != 0,
    })
}

fn read_target(reader: &mut Reader<'_>) -> Result<CallTarget, Corrupt> {
    match reader.number()? {
        TARGET_CONSTRUCTOR => Ok(CallTarget::Constructor),
        TARGET_METHOD => Ok(CallTarget::Method),
        TARGET_IMPORTED_METHOD => Ok(CallTarget::ImportedMethod),
        _ => Err(Corrupt),
    }
}

pub fn write_nested_nulls(writer: &mut Writer, nested_nulls: &[NestedNull]) {
    writer.list(nested_nulls, |writer, nested| {
        write_null_argument(writer, &nested.inner);
        write_null_argument(writer, &nested.outer);
    });
}

pub fn read_nested_nulls(reader: &mut Reader<'_>) -> Result<Vec<NestedNull>, Corrupt> {
    reader.list(|reader| {
        Ok(NestedNull {
            inner: read_null_argument(reader)?,
            outer: read_null_argument(reader)?,
        })
    })
}

pub fn write_references(writer: &mut Writer, references: &[ReferenceArgument]) {
    writer.list(references, |writer, reference| {
        write_null_argument(writer, &reference.argument);
        writer.text(&reference.owner);
        writer.text(&reference.method);
    });
}

pub fn read_references(reader: &mut Reader<'_>) -> Result<Vec<ReferenceArgument>, Corrupt> {
    reader.list(|reader| {
        Ok(ReferenceArgument {
            argument: read_null_argument(reader)?,
            owner: reader.text()?,
            method: reader.text()?,
        })
    })
}
