use super::codec::{Corrupt, Reader, Writer};
use super::summary_codec::{read_flag, write_flag};
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_null_flow::{Dereference, NullFlow};

const ABSENT: u64 = 0;
const PRESENT: u64 = 1;
const ROOT_INSTANCE: u64 = 0;
const ROOT_CLASS: u64 = 1;

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
            })
        })?,
        unboxed: match reader.number()? {
            ABSENT => None,
            PRESENT => Some((reader.size()?, reader.size()?)),
            _ => return Err(Corrupt),
        },
    })
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
