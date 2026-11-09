use super::codec::{Corrupt, Reader, Writer};
use crate::lint::java_generics::{GenericParent, Generics, MethodGenerics, TypeGenerics};

pub fn write_generics(writer: &mut Writer, generics: &Generics) {
    writer.list(&generics.types, |writer, declared| {
        writer.text(&declared.name);
        writer.list(&declared.parameters, |writer, name| writer.text(name));
        writer.list(&declared.parents, |writer, parent| {
            writer.text(&parent.written);
            writer.list(&parent.arguments, |writer, argument| writer.text(argument));
        });
    });
    writer.list(&generics.methods, |writer, method| {
        writer.size(method.line);
        writer.size(method.column);
        writer.list(&method.parameters, |writer, name| writer.text(name));
    });
}

pub fn read_generics(reader: &mut Reader<'_>) -> Result<Generics, Corrupt> {
    Ok(Generics {
        types: reader.list(|reader| {
            Ok(TypeGenerics {
                name: reader.text()?,
                parameters: reader.list(Reader::text)?,
                parents: reader.list(|reader| {
                    Ok(GenericParent {
                        written: reader.text()?,
                        arguments: reader.list(Reader::text)?,
                    })
                })?,
            })
        })?,
        methods: reader.list(|reader| {
            Ok(MethodGenerics {
                line: reader.size()?,
                column: reader.size()?,
                parameters: reader.list(Reader::text)?,
            })
        })?,
    })
}

#[cfg(test)]
mod tests;
