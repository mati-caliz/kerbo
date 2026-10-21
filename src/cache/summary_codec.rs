use super::codec::{Corrupt, Reader, Writer};
use crate::index::java::{CallSite, Declaration, JavaSummary, Parameter, Position};
use crate::lint::java_calls::{Arity, Body, Returns, Visibility};
use crate::lint::java_constants::Constant;

const ABSENT: u64 = 0;
const PRESENT: u64 = 1;
const NO: u64 = 0;
const YES: u64 = 1;
const PRIVATE: u64 = 0;
const PACKAGE: u64 = 1;
const PROTECTED: u64 = 2;
const PUBLIC: u64 = 3;
const RETURNS_NOTHING: u64 = 0;
const RETURNS_ALWAYS: u64 = 1;
const RETURNS_VARIED: u64 = 2;

pub fn write_summary(writer: &mut Writer, summary: Option<&JavaSummary>) {
    let Some(summary) = summary else {
        writer.number(ABSENT);
        return;
    };
    writer.number(PRESENT);
    writer.list(&summary.declarations, write_declaration);
    writer.list(&summary.calls, |writer, call| {
        writer.text(&call.name);
        write_flag(writer, call.is_constructor);
        writer.list(&call.arguments, |writer, argument| {
            write_optional_text(writer, argument.as_deref());
        });
        write_flag(writer, call.result_used);
    });
    writer.list(&summary.referenced, |writer, name| writer.text(name));
    writer.list(&summary.types, |writer, name| writer.text(name));
    writer.text(&summary.package);
    writer.list(&summary.constants, |writer, constant| {
        writer.text(&constant.owner);
        writer.text(&constant.name);
        write_visibility(writer, constant.visibility);
    });
}

pub fn read_summary(reader: &mut Reader<'_>) -> Result<Option<JavaSummary>, Corrupt> {
    if reader.number()? == ABSENT {
        return Ok(None);
    }
    Ok(Some(JavaSummary {
        declarations: reader.list(read_declaration)?,
        calls: reader.list(|reader| {
            Ok(CallSite {
                name: reader.text()?,
                is_constructor: read_flag(reader)?,
                arguments: reader.list(read_optional_text)?,
                result_used: read_flag(reader)?,
            })
        })?,
        referenced: reader.list(Reader::text)?,
        types: reader.list(Reader::text)?,
        package: reader.text()?,
        constants: reader.list(|reader| {
            Ok(Constant {
                owner: reader.text()?,
                name: reader.text()?,
                visibility: read_visibility(reader)?,
            })
        })?,
    }))
}

fn write_declaration(writer: &mut Writer, declaration: &Declaration) {
    writer.text(&declaration.name);
    writer.text(&declaration.scope);
    write_position(writer, &declaration.position);
    write_flag(writer, declaration.is_constructor);
    write_visibility(writer, declaration.visibility);
    write_flag(writer, declaration.overrides);
    write_flag(writer, declaration.body == Body::Written);
    write_flag(writer, declaration.arity == Arity::Variadic);
    match &declaration.returns {
        Returns::Nothing => writer.number(RETURNS_NOTHING),
        Returns::Always(value) => {
            writer.number(RETURNS_ALWAYS);
            writer.text(value);
        }
        Returns::Varied => writer.number(RETURNS_VARIED),
    }
    writer.list(&declaration.parameters, |writer, parameter| {
        writer.text(&parameter.name);
        write_position(writer, &parameter.position);
    });
}

fn read_declaration(reader: &mut Reader<'_>) -> Result<Declaration, Corrupt> {
    Ok(Declaration {
        name: reader.text()?,
        scope: reader.text()?,
        position: read_position(reader)?,
        is_constructor: read_flag(reader)?,
        visibility: read_visibility(reader)?,
        overrides: read_flag(reader)?,
        body: if read_flag(reader)? {
            Body::Written
        } else {
            Body::Abstract
        },
        arity: if read_flag(reader)? {
            Arity::Variadic
        } else {
            Arity::Fixed
        },
        returns: match reader.number()? {
            RETURNS_NOTHING => Returns::Nothing,
            RETURNS_ALWAYS => Returns::Always(reader.text()?),
            RETURNS_VARIED => Returns::Varied,
            _ => return Err(Corrupt),
        },
        parameters: reader.list(|reader| {
            Ok(Parameter {
                name: reader.text()?,
                position: read_position(reader)?,
            })
        })?,
    })
}

fn write_visibility(writer: &mut Writer, visibility: Visibility) {
    writer.number(match visibility {
        Visibility::Private => PRIVATE,
        Visibility::Package => PACKAGE,
        Visibility::Protected => PROTECTED,
        Visibility::Public => PUBLIC,
    });
}

fn read_visibility(reader: &mut Reader<'_>) -> Result<Visibility, Corrupt> {
    match reader.number()? {
        PRIVATE => Ok(Visibility::Private),
        PACKAGE => Ok(Visibility::Package),
        PROTECTED => Ok(Visibility::Protected),
        PUBLIC => Ok(Visibility::Public),
        _ => Err(Corrupt),
    }
}

fn write_position(writer: &mut Writer, position: &Position) {
    writer.size(position.line);
    writer.size(position.column);
}

fn read_position(reader: &mut Reader<'_>) -> Result<Position, Corrupt> {
    Ok(Position {
        line: reader.size()?,
        column: reader.size()?,
    })
}

fn write_flag(writer: &mut Writer, flag: bool) {
    writer.number(if flag { YES } else { NO });
}

fn read_flag(reader: &mut Reader<'_>) -> Result<bool, Corrupt> {
    match reader.number()? {
        NO => Ok(false),
        YES => Ok(true),
        _ => Err(Corrupt),
    }
}

fn write_optional_text(writer: &mut Writer, value: Option<&str>) {
    match value {
        Some(value) => {
            writer.number(PRESENT);
            writer.text(value);
        }
        None => writer.number(ABSENT),
    }
}

fn read_optional_text(reader: &mut Reader<'_>) -> Result<Option<String>, Corrupt> {
    match reader.number()? {
        ABSENT => Ok(None),
        PRESENT => reader.text().map(Some),
        _ => Err(Corrupt),
    }
}
