use super::codec::{Corrupt, Reader, Writer};
use crate::lint::go::{CapitalizedError, GoPackageFacts};

const ABSENT: u64 = 0;
const PRESENT: u64 = 1;

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
        })),
        _ => Err(Corrupt),
    }
}
