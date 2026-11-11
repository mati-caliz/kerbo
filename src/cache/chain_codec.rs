use super::codec::{Corrupt, Reader, Writer};
use super::summary_codec::{read_flag, write_flag};
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_null_flow::{Dereference, NullFlow};
use crate::lint::java_resources::ResourceOrigin;

const ABSENT: u64 = 0;
const PRESENT: u64 = 1;
const ROOT_INSTANCE: u64 = 0;
const ROOT_CLASS: u64 = 1;
const ORIGIN_CALL: u64 = 0;
const ORIGIN_CONSTRUCTION: u64 = 1;

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
    });
    match chain.unboxed {
        Some((line, column)) => {
            writer.number(PRESENT);
            writer.size(line);
            writer.size(column);
        }
        None => writer.number(ABSENT),
    }
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
            })
        })?,
        unboxed: match reader.number()? {
            ABSENT => None,
            PRESENT => Some((reader.size()?, reader.size()?)),
            _ => return Err(Corrupt),
        },
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
    write_chain(writer, &flow.origin);
    writer.list(&flow.dereferences, |writer, used| {
        writer.text(&used.name);
        writer.size(used.line);
        writer.size(used.column);
    });
}

pub(super) fn read_flow(reader: &mut Reader<'_>) -> Result<NullFlow, Corrupt> {
    Ok(NullFlow {
        origin: read_chain(reader)?,
        dereferences: reader.list(|reader| {
            Ok(Dereference {
                name: reader.text()?,
                line: reader.size()?,
                column: reader.size()?,
            })
        })?,
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
