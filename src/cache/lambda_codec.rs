use super::chain_codec::{read_null_return, write_null_return};
use super::codec::{Corrupt, Reader, Writer};
use super::null_codec::{read_null_argument, write_null_argument};
use super::summary_codec::{read_flag, write_flag};
use crate::lint::java_lambda_returns::{LambdaReturn, LambdaTarget};
use crate::lint::java_lambda_targets::{LambdaSite, LambdaTargets};
use crate::lint::java_type_arguments::TypeArgument;

const TARGET_ARGUMENT: u64 = 0;
const TARGET_DECLARED: u64 = 1;
const ARGUMENT_CONCRETE: u64 = 0;
const ARGUMENT_NULLABLE: u64 = 1;
const ARGUMENT_METHOD_VARIABLE: u64 = 2;
const ARGUMENT_OTHER_VARIABLE: u64 = 3;
const ARGUMENT_WILDCARD: u64 = 4;

pub fn write_lambda_targets(writer: &mut Writer, targets: &LambdaTargets) {
    writer.list(&targets.sites, |writer, site| {
        writer.optional_size(site.arity);
        writer.list(site.argument_of.as_slice(), |writer, name| writer.text(name));
    });
    writer.list(&targets.named_types, |writer, name| writer.text(name));
    writer.list(&targets.parameter_types, |writer, (method, written)| {
        writer.text(method);
        writer.text(written);
    });
}

pub fn read_lambda_targets(reader: &mut Reader<'_>) -> Result<LambdaTargets, Corrupt> {
    Ok(LambdaTargets {
        sites: reader.list(|reader| {
            Ok(LambdaSite {
                arity: reader.optional_size()?,
                argument_of: reader.list(Reader::text)?.into_iter().next(),
            })
        })?,
        named_types: reader.list(Reader::text)?,
        parameter_types: reader.list(|reader| Ok((reader.text()?, reader.text()?)))?,
    })
}

pub fn write_lambda_return(writer: &mut Writer, lambda: &LambdaReturn) {
    writer.list(&lambda.returns, write_null_return);
    match &lambda.target {
        LambdaTarget::Argument {
            argument,
            target_typed,
        } => {
            writer.number(TARGET_ARGUMENT);
            write_null_argument(writer, argument);
            write_flag(writer, *target_typed);
        }
        LambdaTarget::Declared { type_name, arguments } => {
            writer.number(TARGET_DECLARED);
            writer.text(type_name);
            write_type_arguments(writer, arguments);
        }
    }
}

pub fn read_lambda_return(reader: &mut Reader<'_>) -> Result<LambdaReturn, Corrupt> {
    Ok(LambdaReturn {
        returns: reader.list(read_null_return)?,
        target: match reader.number()? {
            TARGET_ARGUMENT => LambdaTarget::Argument {
                argument: read_null_argument(reader)?,
                target_typed: read_flag(reader)?,
            },
            TARGET_DECLARED => LambdaTarget::Declared {
                type_name: reader.text()?,
                arguments: read_type_arguments(reader)?,
            },
            _ => return Err(Corrupt),
        },
    })
}

pub fn write_type_arguments(writer: &mut Writer, arguments: &[TypeArgument]) {
    writer.list(arguments, |writer, argument| {
        writer.number(match argument {
            TypeArgument::Concrete => ARGUMENT_CONCRETE,
            TypeArgument::Nullable => ARGUMENT_NULLABLE,
            TypeArgument::MethodVariable => ARGUMENT_METHOD_VARIABLE,
            TypeArgument::OtherVariable(_) => ARGUMENT_OTHER_VARIABLE,
            TypeArgument::Wildcard => ARGUMENT_WILDCARD,
        });
        if let TypeArgument::OtherVariable(name) = argument {
            writer.text(name);
        }
    });
}

pub fn read_type_arguments(reader: &mut Reader<'_>) -> Result<Vec<TypeArgument>, Corrupt> {
    reader.list(|reader| {
        Ok(match reader.number()? {
            ARGUMENT_CONCRETE => TypeArgument::Concrete,
            ARGUMENT_NULLABLE => TypeArgument::Nullable,
            ARGUMENT_METHOD_VARIABLE => TypeArgument::MethodVariable,
            ARGUMENT_OTHER_VARIABLE => TypeArgument::OtherVariable(reader.text()?),
            ARGUMENT_WILDCARD => TypeArgument::Wildcard,
            _ => return Err(Corrupt),
        })
    })
}
