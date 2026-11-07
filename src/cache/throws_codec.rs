use super::codec::{Corrupt, Reader, Writer};
use crate::lint::java_caught_calls::CaughtCall;
use crate::lint::java_throws::{CallRoot, CallStep, MethodThrows, ThrowOrigin, ThrowPoint, ThrowsClause};

const ORIGIN_UNKNOWN: u64 = 0;
const ORIGIN_THROWN: u64 = 1;
const ORIGIN_CREATION: u64 = 2;
const ORIGIN_CLOSE: u64 = 3;
const ORIGIN_CALL: u64 = 4;
const ORIGIN_RETHROWN: u64 = 5;
const ROOT_SCOPE: u64 = 0;
const ROOT_TYPE: u64 = 1;
const ROOT_GENERIC: u64 = 2;

pub fn write_method_throws(writer: &mut Writer, method: &MethodThrows) {
    writer.size(method.line);
    writer.size(method.column);
    writer.list(&method.clauses, |writer, clause| {
        writer.text(&clause.written);
        writer.size(clause.line);
        writer.size(clause.column);
    });
    writer.list(&method.points, |writer, point| {
        write_origin(writer, &point.origin);
        writer.list(&point.caught, |writer, name| writer.text(name));
    });
}

pub fn read_method_throws(reader: &mut Reader<'_>) -> Result<MethodThrows, Corrupt> {
    Ok(MethodThrows {
        line: reader.size()?,
        column: reader.size()?,
        clauses: reader.list(|reader| {
            Ok(ThrowsClause {
                written: reader.text()?,
                line: reader.size()?,
                column: reader.size()?,
            })
        })?,
        points: reader.list(|reader| {
            Ok(ThrowPoint {
                origin: read_origin(reader)?,
                caught: reader.list(Reader::text)?,
            })
        })?,
    })
}

pub fn write_caught_call(writer: &mut Writer, call: &CaughtCall) {
    writer.text(&call.name);
    writer.list(&call.caught, |writer, name| writer.text(name));
    write_origin(writer, &call.origin);
}

pub fn read_caught_call(reader: &mut Reader<'_>) -> Result<CaughtCall, Corrupt> {
    Ok(CaughtCall {
        name: reader.text()?,
        caught: reader.list(Reader::text)?,
        origin: read_origin(reader)?,
    })
}

fn write_origin(writer: &mut Writer, origin: &ThrowOrigin) {
    match origin {
        ThrowOrigin::Unknown => writer.number(ORIGIN_UNKNOWN),
        ThrowOrigin::Thrown(name) => {
            writer.number(ORIGIN_THROWN);
            writer.text(name);
        }
        ThrowOrigin::Creation(name, arity) => {
            writer.number(ORIGIN_CREATION);
            writer.text(name);
            writer.size(*arity);
        }
        ThrowOrigin::Close(name) => {
            writer.number(ORIGIN_CLOSE);
            writer.text(name);
        }
        ThrowOrigin::Rethrown(name) => {
            writer.number(ORIGIN_RETHROWN);
            writer.text(name);
        }
        ThrowOrigin::Call(root, steps) => {
            writer.number(ORIGIN_CALL);
            write_root(writer, root);
            writer.list(steps, |writer, step| {
                writer.text(&step.name);
                writer.size(step.arity);
            });
        }
    }
}

fn read_origin(reader: &mut Reader<'_>) -> Result<ThrowOrigin, Corrupt> {
    match reader.number()? {
        ORIGIN_UNKNOWN => Ok(ThrowOrigin::Unknown),
        ORIGIN_THROWN => Ok(ThrowOrigin::Thrown(reader.text()?)),
        ORIGIN_CREATION => Ok(ThrowOrigin::Creation(reader.text()?, reader.size()?)),
        ORIGIN_CLOSE => Ok(ThrowOrigin::Close(reader.text()?)),
        ORIGIN_RETHROWN => Ok(ThrowOrigin::Rethrown(reader.text()?)),
        ORIGIN_CALL => Ok(ThrowOrigin::Call(
            read_root(reader)?,
            reader.list(|reader| {
                Ok(CallStep {
                    name: reader.text()?,
                    arity: reader.size()?,
                })
            })?,
        )),
        _ => Err(Corrupt),
    }
}

fn write_root(writer: &mut Writer, root: &CallRoot) {
    match root {
        CallRoot::Scope(enclosing) => {
            writer.number(ROOT_SCOPE);
            writer.list(enclosing, |writer, name| writer.text(name));
        }
        CallRoot::Type(name) => {
            writer.number(ROOT_TYPE);
            writer.text(name);
        }
        CallRoot::Generic(name, arguments) => {
            writer.number(ROOT_GENERIC);
            writer.text(name);
            writer.list(arguments, |writer, argument| writer.text(argument));
        }
    }
}

fn read_root(reader: &mut Reader<'_>) -> Result<CallRoot, Corrupt> {
    match reader.number()? {
        ROOT_SCOPE => Ok(CallRoot::Scope(reader.list(Reader::text)?)),
        ROOT_TYPE => Ok(CallRoot::Type(reader.text()?)),
        ROOT_GENERIC => Ok(CallRoot::Generic(reader.text()?, reader.list(Reader::text)?)),
        _ => Err(Corrupt),
    }
}
