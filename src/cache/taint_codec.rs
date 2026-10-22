use super::codec::{Corrupt, Reader, Writer};
use super::summary_codec::{read_receiver, write_receiver};
use crate::lint::java_taint::{Atom, Flow, SinkReceiver, TaintCall, TaintMethod, TaintSink};

const ATOM_SOURCE: u64 = 0;
const ATOM_PARAMETER: u64 = 1;
const ATOM_CALL: u64 = 2;
const SINK_TYPE: u64 = 0;
const SINK_STATIC_FIELD: u64 = 1;
const SINK_CONSTRUCTOR: u64 = 2;

pub fn write_taints(writer: &mut Writer, taints: &[TaintMethod]) {
    writer.list(taints, |writer, method| {
        writer.text(&method.name);
        writer.text(&method.owner);
        writer.size(method.arity);
        write_flow(writer, &method.returns);
        writer.list(&method.calls, |writer, call| {
            writer.text(&call.name);
            write_receiver(writer, &call.receiver);
            write_flow(writer, &call.receiver_flow);
            writer.list(&call.arguments, write_flow);
        });
        writer.list(&method.sinks, |writer, sink| {
            writer.text(&sink.callee);
            write_sink_receiver(writer, &sink.receiver);
            writer.list(&sink.arguments, write_flow);
            writer.size(sink.line);
            writer.size(sink.column);
        });
    });
}

pub fn read_taints(reader: &mut Reader<'_>) -> Result<Vec<TaintMethod>, Corrupt> {
    reader.list(|reader| {
        Ok(TaintMethod {
            name: reader.text()?,
            owner: reader.text()?,
            arity: reader.size()?,
            returns: read_flow(reader)?,
            calls: reader.list(|reader| {
                Ok(TaintCall {
                    name: reader.text()?,
                    receiver: read_receiver(reader)?,
                    receiver_flow: read_flow(reader)?,
                    arguments: reader.list(read_flow)?,
                })
            })?,
            sinks: reader.list(|reader| {
                Ok(TaintSink {
                    callee: reader.text()?,
                    receiver: read_sink_receiver(reader)?,
                    arguments: reader.list(read_flow)?,
                    line: reader.size()?,
                    column: reader.size()?,
                })
            })?,
        })
    })
}

fn write_flow(writer: &mut Writer, flow: &Flow) {
    writer.list(&flow.atoms, write_atom);
    writer.list(&flow.escaped, write_atom);
}

fn write_atom(writer: &mut Writer, atom: &Atom) {
    match atom {
        Atom::Source => writer.number(ATOM_SOURCE),
        Atom::Parameter(position) => {
            writer.number(ATOM_PARAMETER);
            writer.size(*position);
        }
        Atom::Call(index) => {
            writer.number(ATOM_CALL);
            writer.size(*index);
        }
    }
}

fn read_flow(reader: &mut Reader<'_>) -> Result<Flow, Corrupt> {
    Ok(Flow {
        atoms: reader.list(read_atom)?,
        escaped: reader.list(read_atom)?,
    })
}

fn read_atom(reader: &mut Reader<'_>) -> Result<Atom, Corrupt> {
    match reader.number()? {
        ATOM_SOURCE => Ok(Atom::Source),
        ATOM_PARAMETER => Ok(Atom::Parameter(reader.size()?)),
        ATOM_CALL => Ok(Atom::Call(reader.size()?)),
        _ => Err(Corrupt),
    }
}

fn write_sink_receiver(writer: &mut Writer, receiver: &SinkReceiver) {
    match receiver {
        SinkReceiver::Type(name) => {
            writer.number(SINK_TYPE);
            writer.text(name);
        }
        SinkReceiver::StaticField { owner, field } => {
            writer.number(SINK_STATIC_FIELD);
            writer.text(owner);
            writer.text(field);
        }
        SinkReceiver::Constructor(created) => {
            writer.number(SINK_CONSTRUCTOR);
            writer.text(created);
        }
    }
}

fn read_sink_receiver(reader: &mut Reader<'_>) -> Result<SinkReceiver, Corrupt> {
    match reader.number()? {
        SINK_TYPE => Ok(SinkReceiver::Type(reader.text()?)),
        SINK_STATIC_FIELD => Ok(SinkReceiver::StaticField {
            owner: reader.text()?,
            field: reader.text()?,
        }),
        SINK_CONSTRUCTOR => Ok(SinkReceiver::Constructor(reader.text()?)),
        _ => Err(Corrupt),
    }
}
