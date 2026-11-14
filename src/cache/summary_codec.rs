use super::chain_codec::{
    read_chain, read_flow, read_resource_origin, write_chain, write_flow, write_resource_origin,
};
use super::codec::{Corrupt, Reader, Writer};
use super::declaration_codec::{read_declaration, read_owner, write_declaration, write_owner};
use super::generics_codec::{read_generics, write_generics};
use super::lambda_codec::{
    read_lambda_return, read_lambda_targets, write_lambda_return, write_lambda_targets,
};
use super::null_codec::{read_contract_call, read_null_argument, write_contract_call, write_null_argument};
use super::taint_codec::{read_taints, write_taints};
use super::throws_codec::{read_method_throws, write_method_throws};
use crate::index::java::{CallSite, Field, JavaSummary, Position};
use crate::lint::java_nested_arguments::NestedNull;

use crate::index::java_exposure::{DeclaredType, Exposure};
use crate::lint::java_calls::{ResultUse, Visibility};
use crate::lint::java_constants::Constant;
use crate::lint::java_fields::Volatility;
use crate::lint::java_receivers::Receiver;
use crate::lint::java_resources::ResourceCandidate;

const RESULT_DISCARDED: u64 = 0;
const RESULT_USED: u64 = 1;
const RESULT_NEGATED: u64 = 2;
const ABSENT: u64 = 0;
const PRESENT: u64 = 1;
const NO: u64 = 0;
const YES: u64 = 1;
const PRIVATE: u64 = 0;
const PACKAGE: u64 = 1;
const PROTECTED: u64 = 2;
const PUBLIC: u64 = 3;
const RECEIVER_UNKNOWN: u64 = 0;
const RECEIVER_TYPE: u64 = 1;
const RECEIVER_SCOPE: u64 = 2;

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
        write_receiver(writer, &call.receiver);
        writer.list(&call.arguments, |writer, argument| {
            write_optional_text(writer, argument.as_deref());
        });
        writer.number(match call.result {
            ResultUse::Discarded => RESULT_DISCARDED,
            ResultUse::Used => RESULT_USED,
            ResultUse::Negated => RESULT_NEGATED,
        });
    });
    writer.list(&summary.referenced, |writer, name| writer.text(name));
    writer.list(&summary.types, |writer, name| writer.text(name));
    writer.text(&summary.package);
    writer.list(&summary.mentioned, |writer, name| writer.text(name));
    writer.list(&summary.hierarchy, |writer, (name, supertypes)| {
        writer.text(name);
        writer.list(supertypes, |writer, supertype| writer.text(supertype));
    });
    writer.list(&summary.imports, |writer, path| writer.text(path));
    writer.list(&summary.package_annotations, |writer, name| writer.text(name));
    writer.list(&summary.chains, write_chain);
    writer.list(&summary.null_flows, write_flow);
    writer.list(&summary.resources, |writer, candidate| {
        write_resource_origin(writer, &candidate.origin);
        writer.list(&candidate.owners, write_resource_origin);
        writer.size(candidate.line);
        writer.size(candidate.column);
    });
    writer.list(&summary.constants, |writer, constant| {
        writer.text(&constant.owner);
        writer.text(&constant.name);
        write_visibility(writer, constant.visibility);
    });
    writer.list(&summary.fields, |writer, field| {
        writer.text(&field.name);
        write_position(writer, &field.position);
        write_visibility(writer, field.visibility);
        write_flag(writer, field.is_static);
        writer.text(&field.type_name);
        writer.list(&field.annotations, |writer, name| writer.text(name));
        write_owner(writer, &field.owner);
        write_flag(writer, field.is_nullable);
        write_flag(writer, field.volatility == Volatility::Volatile);
    });
    writer.list(&summary.identifiers, |writer, name| writer.text(name));
    writer.list(&summary.written_only, |writer, name| writer.text(name));
    write_taints(writer, &summary.taints);
    writer.list(&summary.declared_types, |writer, declared| {
        writer.text(&declared.name);
        write_flag(writer, declared.top_level);
        write_visibility(writer, declared.reach);
    });
    writer.list(&summary.exposures, |writer, exposure| {
        writer.text(&exposure.written);
        write_position(writer, &exposure.position);
        write_visibility(writer, exposure.reach);
        writer.list(&exposure.type_parameters, |writer, name| writer.text(name));
    });
    writer.list(&summary.static_imports, |writer, path| writer.text(path));
    writer.list(&summary.method_throws, write_method_throws);
    writer.list(&summary.null_arguments, write_null_argument);
    writer.list(&summary.contract_calls, write_contract_call);
    write_generics(writer, &summary.generics);
    write_lambda_targets(writer, &summary.lambda_targets);
    writer.list(&summary.lambda_returns, write_lambda_return);
    writer.list(&summary.nested_nulls, |writer, nested| {
        write_null_argument(writer, &nested.inner);
        write_null_argument(writer, &nested.outer);
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
                receiver: read_receiver(reader)?,
                arguments: reader.list(read_optional_text)?,
                result: match reader.number()? {
                    RESULT_DISCARDED => ResultUse::Discarded,
                    RESULT_USED => ResultUse::Used,
                    RESULT_NEGATED => ResultUse::Negated,
                    _ => return Err(Corrupt),
                },
            })
        })?,
        referenced: reader.list(Reader::text)?,
        types: reader.list(Reader::text)?,
        package: reader.text()?,
        mentioned: reader.list(Reader::text)?,
        hierarchy: reader.list(|reader| Ok((reader.text()?, reader.list(Reader::text)?)))?,
        imports: reader.list(Reader::text)?,
        package_annotations: reader.list(Reader::text)?,
        chains: reader.list(read_chain)?,
        null_flows: reader.list(read_flow)?,
        resources: reader.list(|reader| {
            Ok(ResourceCandidate {
                origin: read_resource_origin(reader)?,
                owners: reader.list(read_resource_origin)?,
                line: reader.size()?,
                column: reader.size()?,
            })
        })?,
        constants: reader.list(|reader| {
            Ok(Constant {
                owner: reader.text()?,
                name: reader.text()?,
                visibility: read_visibility(reader)?,
            })
        })?,
        fields: reader.list(read_field)?,
        identifiers: reader.list(Reader::text)?,
        written_only: reader.list(Reader::text)?,
        taints: read_taints(reader)?,
        declared_types: reader.list(|reader| {
            Ok(DeclaredType {
                name: reader.text()?,
                top_level: read_flag(reader)?,
                reach: read_visibility(reader)?,
            })
        })?,
        exposures: reader.list(|reader| {
            Ok(Exposure {
                written: reader.text()?,
                position: read_position(reader)?,
                reach: read_visibility(reader)?,
                type_parameters: reader.list(Reader::text)?,
            })
        })?,
        static_imports: reader.list(Reader::text)?,
        method_throws: reader.list(read_method_throws)?,
        null_arguments: reader.list(read_null_argument)?,
        contract_calls: reader.list(read_contract_call)?,
        generics: read_generics(reader)?,
        lambda_targets: read_lambda_targets(reader)?,
        lambda_returns: reader.list(read_lambda_return)?,
        nested_nulls: reader.list(|reader| {
            Ok(NestedNull {
                inner: read_null_argument(reader)?,
                outer: read_null_argument(reader)?,
            })
        })?,
    }))
}

pub(super) fn write_visibility(writer: &mut Writer, visibility: Visibility) {
    writer.number(match visibility {
        Visibility::Private => PRIVATE,
        Visibility::Package => PACKAGE,
        Visibility::Protected => PROTECTED,
        Visibility::Public => PUBLIC,
    });
}

pub(super) fn read_visibility(reader: &mut Reader<'_>) -> Result<Visibility, Corrupt> {
    match reader.number()? {
        PRIVATE => Ok(Visibility::Private),
        PACKAGE => Ok(Visibility::Package),
        PROTECTED => Ok(Visibility::Protected),
        PUBLIC => Ok(Visibility::Public),
        _ => Err(Corrupt),
    }
}

fn read_field(reader: &mut Reader<'_>) -> Result<Field, Corrupt> {
    Ok(Field {
        name: reader.text()?,
        position: read_position(reader)?,
        visibility: read_visibility(reader)?,
        is_static: read_flag(reader)?,
        type_name: reader.text()?,
        annotations: reader.list(Reader::text)?,
        owner: read_owner(reader)?,
        is_nullable: read_flag(reader)?,
        volatility: if read_flag(reader)? {
            Volatility::Volatile
        } else {
            Volatility::Stable
        },
    })
}

pub(super) fn write_receiver(writer: &mut Writer, receiver: &Receiver) {
    match receiver {
        Receiver::Unknown => writer.number(RECEIVER_UNKNOWN),
        Receiver::Type(name) => {
            writer.number(RECEIVER_TYPE);
            writer.text(name);
        }
        Receiver::Scope(enclosing) => {
            writer.number(RECEIVER_SCOPE);
            writer.list(enclosing, |writer, name| writer.text(name));
        }
    }
}

pub(super) fn read_receiver(reader: &mut Reader<'_>) -> Result<Receiver, Corrupt> {
    match reader.number()? {
        RECEIVER_UNKNOWN => Ok(Receiver::Unknown),
        RECEIVER_TYPE => Ok(Receiver::Type(reader.text()?)),
        RECEIVER_SCOPE => Ok(Receiver::Scope(reader.list(Reader::text)?)),
        _ => Err(Corrupt),
    }
}

pub(super) fn write_position(writer: &mut Writer, position: &Position) {
    writer.size(position.line);
    writer.size(position.column);
}

pub(super) fn read_position(reader: &mut Reader<'_>) -> Result<Position, Corrupt> {
    Ok(Position {
        line: reader.size()?,
        column: reader.size()?,
    })
}

pub(super) fn write_flag(writer: &mut Writer, flag: bool) {
    writer.number(if flag { YES } else { NO });
}

pub(super) fn read_flag(reader: &mut Reader<'_>) -> Result<bool, Corrupt> {
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
