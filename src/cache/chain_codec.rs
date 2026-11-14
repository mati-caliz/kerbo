use super::codec::{Corrupt, Reader, Writer};
use super::declaration_codec::{read_marking, write_marking};
use super::null_codec::{read_null_argument, write_null_argument};
use super::summary_codec::{read_flag, write_flag};
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_null_flow::{Dereference, NullFlow, NullOrigin, NullReturn};
use crate::lint::java_resources::ResourceOrigin;

const ABSENT: u64 = 0;
const PRESENT: u64 = 1;
const ROOT_INSTANCE: u64 = 0;
const ROOT_CLASS: u64 = 1;
const ORIGIN_CALL: u64 = 0;
const ORIGIN_CONSTRUCTION: u64 = 1;
const NULL_FROM_CALL: u64 = 0;
const NULL_FROM_FIELD: u64 = 1;
const NULL_FROM_QUALIFIED_FIELD: u64 = 2;
const NULL_FROM_LITERAL: u64 = 3;

pub(super) fn write_chain(writer: &mut Writer, chain: &Chain) {
    match &chain.root {
        ChainRoot::Instance(name) => {
            writer.number(ROOT_INSTANCE);
            writer.text(name);
        }
        ChainRoot::Class(name) => {
            writer.number(ROOT_CLASS);
            writer.text(name);
        }
    }
    writer.list(&chain.steps, |writer, step| {
        writer.text(&step.name);
        writer.size(step.arity);
        writer.size(step.line);
        writer.size(step.column);
        write_flag(writer, step.checked);
        writer.list(&step.class_arguments, |writer, position| writer.size(*position));
        writer.list(&step.text_arguments, |writer, argument| writer.text(argument));
        writer.list(&step.argument_types, |writer, argument| match argument {
            Some(declared) => {
                writer.number(PRESENT);
                writer.text(declared);
            }
            None => writer.number(ABSENT),
        });
        writer.list(&step.passed_before, write_null_argument);
    });
    match chain.dereferenced_at {
        Some((line, column)) => {
            writer.number(PRESENT);
            writer.size(line);
            writer.size(column);
        }
        None => writer.number(ABSENT),
    }
    match &chain.passed_as {
        Some(argument) => {
            writer.number(PRESENT);
            write_null_argument(writer, argument);
        }
        None => writer.number(ABSENT),
    }
    writer.list(chain.returned.as_slice(), write_null_return);
}

pub(super) fn write_null_return(writer: &mut Writer, returned: &NullReturn) {
    writer.text(&returned.name);
    writer.size(returned.line);
    writer.size(returned.column);
    write_marking(writer, returned.marking);
}

pub(super) fn read_null_return(reader: &mut Reader<'_>) -> Result<NullReturn, Corrupt> {
    Ok(NullReturn {
        name: reader.text()?,
        line: reader.size()?,
        column: reader.size()?,
        marking: read_marking(reader)?,
    })
}

pub(super) fn read_chain(reader: &mut Reader<'_>) -> Result<Chain, Corrupt> {
    Ok(Chain {
        root: match reader.number()? {
            ROOT_INSTANCE => ChainRoot::Instance(reader.text()?),
            ROOT_CLASS => ChainRoot::Class(reader.text()?),
            _ => return Err(Corrupt),
        },
        steps: reader.list(|reader| {
            Ok(ChainStep {
                name: reader.text()?,
                arity: reader.size()?,
                line: reader.size()?,
                column: reader.size()?,
                checked: read_flag(reader)?,
                class_arguments: reader.list(Reader::size)?,
                text_arguments: reader.list(Reader::text)?,
                argument_types: reader.list(read_optional_text)?,
                passed_before: reader.list(read_null_argument)?,
            })
        })?,
        dereferenced_at: match reader.number()? {
            ABSENT => None,
            PRESENT => Some((reader.size()?, reader.size()?)),
            _ => return Err(Corrupt),
        },
        passed_as: match reader.number()? {
            ABSENT => None,
            PRESENT => Some(Box::new(read_null_argument(reader)?)),
            _ => return Err(Corrupt),
        },
        returned: reader.list(read_null_return)?.into_iter().next(),
    })
}

fn read_optional_text(reader: &mut Reader<'_>) -> Result<Option<String>, Corrupt> {
    match reader.number()? {
        ABSENT => Ok(None),
        PRESENT => Ok(Some(reader.text()?)),
        _ => Err(Corrupt),
    }
}

pub(super) fn write_flow(writer: &mut Writer, flow: &NullFlow) {
    match &flow.origin {
        NullOrigin::Literal => writer.number(NULL_FROM_LITERAL),
        NullOrigin::Call(chain) => {
            writer.number(NULL_FROM_CALL);
            write_chain(writer, chain);
        }
        NullOrigin::NullableField => writer.number(NULL_FROM_FIELD),
        NullOrigin::QualifiedField {
            owner,
            via,
            field,
            after_call,
        } => {
            writer.number(NULL_FROM_QUALIFIED_FIELD);
            writer.text(owner);
            writer.list(via.as_slice(), |writer, name| writer.text(name));
            writer.text(field);
            writer.number(u64::from(*after_call));
        }
    }
    writer.list(&flow.dereferences, |writer, used| {
        writer.text(&used.name);
        writer.size(used.line);
        writer.size(used.column);
    });
    writer.list(&flow.arguments, write_null_argument);
    writer.list(&flow.passed_before, write_null_argument);
    writer.list(&flow.returns, write_null_return);
}

pub(super) fn read_flow(reader: &mut Reader<'_>) -> Result<NullFlow, Corrupt> {
    Ok(NullFlow {
        origin: match reader.number()? {
            NULL_FROM_LITERAL => NullOrigin::Literal,
            NULL_FROM_CALL => NullOrigin::Call(read_chain(reader)?),
            NULL_FROM_FIELD => NullOrigin::NullableField,
            NULL_FROM_QUALIFIED_FIELD => NullOrigin::QualifiedField {
                owner: reader.text()?,
                via: reader.list(Reader::text)?.into_iter().next(),
                field: reader.text()?,
                after_call: reader.number()? != 0,
            },
            _ => return Err(Corrupt),
        },
        dereferences: reader.list(|reader| {
            Ok(Dereference {
                name: reader.text()?,
                line: reader.size()?,
                column: reader.size()?,
            })
        })?,
        arguments: reader.list(read_null_argument)?,
        passed_before: reader.list(read_null_argument)?,
        returns: reader.list(read_null_return)?,
    })
}

pub(super) fn write_resource_origin(writer: &mut Writer, origin: &ResourceOrigin) {
    match origin {
        ResourceOrigin::Call(chain) => {
            writer.number(ORIGIN_CALL);
            write_chain(writer, chain);
        }
        ResourceOrigin::Construction(class) => {
            writer.number(ORIGIN_CONSTRUCTION);
            writer.text(class);
        }
    }
}

pub(super) fn read_resource_origin(reader: &mut Reader<'_>) -> Result<ResourceOrigin, Corrupt> {
    match reader.number()? {
        ORIGIN_CALL => Ok(ResourceOrigin::Call(read_chain(reader)?)),
        ORIGIN_CONSTRUCTION => Ok(ResourceOrigin::Construction(reader.text()?)),
        _ => Err(Corrupt),
    }
}
