use super::codec::{Corrupt, Reader, Writer};
use crate::lint::go::{CapitalizedError, GoPackageFacts, MessagePiece, PendingMessage};

const ABSENT: u64 = 0;
const PRESENT: u64 = 1;
const TEXT_PIECE: u64 = 0;
const CONSTANT_PIECE: u64 = 1;

pub fn write_go_package(writer: &mut Writer, facts: Option<&GoPackageFacts>) {
    let Some(facts) = facts else {
        writer.number(ABSENT);
        return;
    };
    writer.number(PRESENT);
    writer.text(&facts.package);
    writer.list(&facts.declared, |writer, name| writer.text(name));
    writer.list(&facts.capitalized, |writer, found| {
        writer.size(found.line);
        writer.size(found.column);
        writer.text(&found.word);
    });
    writer.list(&facts.outside_functions, |writer, (line, column)| {
        writer.size(*line);
        writer.size(*column);
    });
    writer.list(&facts.constants, |writer, (name, value)| {
        writer.text(name);
        writer.text(value);
    });
    writer.list(&facts.pending, write_pending);
}

fn write_pending(writer: &mut Writer, pending: &PendingMessage) {
    writer.size(pending.line);
    writer.size(pending.column);
    writer.text(&pending.callee);
    writer.list(&pending.pieces, |writer, piece| match piece {
        MessagePiece::Text(text) => {
            writer.number(TEXT_PIECE);
            writer.text(text);
        }
        MessagePiece::Constant(name) => {
            writer.number(CONSTANT_PIECE);
            writer.text(name);
        }
    });
}

fn read_pending(reader: &mut Reader<'_>) -> Result<PendingMessage, Corrupt> {
    Ok(PendingMessage {
        line: reader.size()?,
        column: reader.size()?,
        callee: reader.text()?,
        pieces: reader.list(|reader| match reader.number()? {
            TEXT_PIECE => Ok(MessagePiece::Text(reader.text()?)),
            CONSTANT_PIECE => Ok(MessagePiece::Constant(reader.text()?)),
            _ => Err(Corrupt),
        })?,
    })
}

pub fn read_go_package(reader: &mut Reader<'_>) -> Result<Option<GoPackageFacts>, Corrupt> {
    match reader.number()? {
        ABSENT => Ok(None),
        PRESENT => Ok(Some(GoPackageFacts {
            package: reader.text()?,
            declared: reader.list(Reader::text)?,
            capitalized: reader.list(|reader| {
                Ok(CapitalizedError {
                    line: reader.size()?,
                    column: reader.size()?,
                    word: reader.text()?,
                })
            })?,
            outside_functions: reader.list(|reader| Ok((reader.size()?, reader.size()?)))?,
            constants: reader.list(|reader| Ok((reader.text()?, reader.text()?)))?,
            pending: reader.list(read_pending)?,
        })),
        _ => Err(Corrupt),
    }
}

#[cfg(test)]
mod tests;
