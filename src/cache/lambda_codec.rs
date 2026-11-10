use super::codec::{Corrupt, Reader, Writer};
use crate::lint::java_lambda_targets::{LambdaSite, LambdaTargets};

pub fn write_lambda_targets(writer: &mut Writer, targets: &LambdaTargets) {
    writer.list(&targets.sites, |writer, site| {
        writer.optional_size(site.arity);
        writer.list(site.argument_of.as_slice(), |writer, name| writer.text(name));
    });
    writer.list(&targets.named_types, |writer, name| writer.text(name));
    writer.list(&targets.parameter_types, |writer, (method, written)| {
        writer.text(method);
        writer.text(written);
    });
}

pub fn read_lambda_targets(reader: &mut Reader<'_>) -> Result<LambdaTargets, Corrupt> {
    Ok(LambdaTargets {
        sites: reader.list(|reader| {
            Ok(LambdaSite {
                arity: reader.optional_size()?,
                argument_of: reader.list(Reader::text)?.into_iter().next(),
            })
        })?,
        named_types: reader.list(Reader::text)?,
        parameter_types: reader.list(|reader| Ok((reader.text()?, reader.text()?)))?,
    })
}
