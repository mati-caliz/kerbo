use super::codec::{Corrupt, Reader, Writer};
use crate::lint::java_throws::{
    ArgumentKind, CallRoot, CallStep, MethodThrows, ThrowOrigin, ThrowPoint, ThrowsClause,
};

const ORIGIN_UNKNOWN: u64 = 0;
const ORIGIN_THROWN: u64 = 1;
const ORIGIN_CREATION: u64 = 2;
const ORIGIN_CLOSE: u64 = 3;
const ORIGIN_CALL: u64 = 4;
const ORIGIN_RETHROWN: u64 = 5;
const ORIGIN_ANONYMOUS: u64 = 6;
const ARGUMENT_UNKNOWN: u64 = 0;
const ARGUMENT_TEXT: u64 = 1;
const ARGUMENT_CHARSET: u64 = 2;
const ARGUMENT_LAMBDA: u64 = 3;
const ARGUMENT_REFERENCE: u64 = 4;
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
    write_points(writer, &method.points);
}

fn write_points(writer: &mut Writer, points: &[ThrowPoint]) {
    writer.list(points, |writer, point| {
        write_origin(writer, &point.origin);
        writer.list(&point.caught, |writer, name| writer.text(name));
    });
}

fn read_points(reader: &mut Reader<'_>) -> Result<Vec<ThrowPoint>, Corrupt> {
    reader.list(|reader| {
        Ok(ThrowPoint {
            origin: read_origin(reader)?,
            caught: reader.list(Reader::text)?,
        })
    })
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
        points: read_points(reader)?,
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
        ThrowOrigin::Anonymous(name) => {
            writer.number(ORIGIN_ANONYMOUS);
            writer.text(name);
        }
        ThrowOrigin::Call(root, steps) => {
            writer.number(ORIGIN_CALL);
            write_root(writer, root);
            writer.list(steps, |writer, step| {
                writer.text(&step.name);
                writer.size(step.arity);
                writer.list(&step.arguments, write_argument);
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
        ORIGIN_ANONYMOUS => Ok(ThrowOrigin::Anonymous(reader.text()?)),
        ORIGIN_CALL => Ok(ThrowOrigin::Call(
            read_root(reader)?,
            reader.list(|reader| {
                Ok(CallStep {
                    name: reader.text()?,
                    arity: reader.size()?,
                    arguments: reader.list(read_argument)?,
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

fn write_argument(writer: &mut Writer, kind: &ArgumentKind) {
    match kind {
        ArgumentKind::Unknown => writer.number(ARGUMENT_UNKNOWN),
        ArgumentKind::Text => writer.number(ARGUMENT_TEXT),
        ArgumentKind::Charset => writer.number(ARGUMENT_CHARSET),
        ArgumentKind::Lambda(points) => {
            writer.number(ARGUMENT_LAMBDA);
            write_points(writer, points);
        }
        ArgumentKind::Reference(root, name) => {
            writer.number(ARGUMENT_REFERENCE);
            write_root(writer, root);
            writer.text(name);
        }
    }
}

fn read_argument(reader: &mut Reader<'_>) -> Result<ArgumentKind, Corrupt> {
    match reader.number()? {
        ARGUMENT_UNKNOWN => Ok(ArgumentKind::Unknown),
        ARGUMENT_TEXT => Ok(ArgumentKind::Text),
        ARGUMENT_CHARSET => Ok(ArgumentKind::Charset),
        ARGUMENT_LAMBDA => Ok(ArgumentKind::Lambda(read_points(reader)?)),
        ARGUMENT_REFERENCE => Ok(ArgumentKind::Reference(read_root(reader)?, reader.text()?)),
        _ => Err(Corrupt),
    }
}
