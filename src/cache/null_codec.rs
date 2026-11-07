use super::codec::{Corrupt, Reader, Writer};
use super::summary_codec::{read_receiver, write_receiver};
use crate::lint::java_null_inference::NullArgument;

pub fn write_null_argument(writer: &mut Writer, argument: &NullArgument) {
    writer.text(&argument.name);
    writer.number(u64::from(argument.is_constructor));
    writer.size(argument.arity);
    write_receiver(writer, &argument.receiver);
    writer.size(argument.index);
    writer.size(argument.line);
    writer.size(argument.column);
}

pub fn read_null_argument(reader: &mut Reader<'_>) -> Result<NullArgument, Corrupt> {
    Ok(NullArgument {
        name: reader.text()?,
        is_constructor: reader.number()? != 0,
        arity: reader.size()?,
        receiver: read_receiver(reader)?,
        index: reader.size()?,
        line: reader.size()?,
        column: reader.size()?,
    })
}
