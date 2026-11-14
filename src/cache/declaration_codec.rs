use super::codec::{Corrupt, Reader, Writer};
use super::lambda_codec::{read_type_arguments, write_type_arguments};
use super::null_codec::{read_constraint, write_constraint};
use super::summary_codec::{
    read_flag, read_position, read_visibility, write_flag, write_position, write_visibility,
};
use crate::index::java::{Declaration, Dereferenced, NullInference, Parameter, ParameterNullness, Reading};
use crate::lint::java_calls::{Arity, Body, ReturnType, Returns};
use crate::lint::java_nullable_fields::ReturnNullness;
use crate::lint::java_owner::{NullMarking, Owner, OwnerKind};

const INFERENCE_OVERRIDABLE: u64 = 0;
const INFERENCE_CLOSED: u64 = 1;
const INFERENCE_RETURNS_NULL: u64 = 2;
const DEREFERENCED_NO: u64 = 0;
const DEREFERENCED_ALWAYS: u64 = 1;
const DEREFERENCED_FIRST: u64 = 2;
const RETURNS_NOTHING: u64 = 0;
const RETURNS_ALWAYS: u64 = 1;
const RETURNS_VARIED: u64 = 2;
const OWNER_CLASS: u64 = 0;
const OWNER_INTERFACE: u64 = 1;
const OWNER_ENUM: u64 = 2;
const OWNER_RECORD: u64 = 3;
const OWNER_ANNOTATION: u64 = 4;
const OWNER_LOCAL: u64 = 5;
const NULLNESS_UNANNOTATED: u64 = 0;
const NULLNESS_NULLABLE: u64 = 1;
const NULLNESS_NULLABLE_VARIABLE: u64 = 2;
const NULLNESS_NON_NULL: u64 = 3;
const MARKING_UNSPECIFIED: u64 = 0;
const MARKING_MARKED: u64 = 1;
const MARKING_UNMARKED: u64 = 2;
const RETURN_TYPE_VOID: u64 = 0;
const RETURN_TYPE_PRIMITIVE: u64 = 1;
const RETURN_TYPE_REFERENCE: u64 = 2;

pub(super) fn write_declaration(writer: &mut Writer, declaration: &Declaration) {
    writer.text(&declaration.name);
    writer.text(&declaration.scope);
    write_position(writer, &declaration.position);
    write_flag(writer, declaration.is_constructor);
    write_visibility(writer, declaration.visibility);
    write_flag(writer, declaration.overrides);
    write_flag(writer, declaration.body == Body::Written);
    write_flag(writer, declaration.arity == Arity::Variadic);
    write_flag(writer, declaration.return_nullness == ReturnNullness::Nullable);
    match &declaration.returns {
        Returns::Nothing => writer.number(RETURNS_NOTHING),
        Returns::Always(value) => {
            writer.number(RETURNS_ALWAYS);
            writer.text(value);
        }
        Returns::Varied => writer.number(RETURNS_VARIED),
    }
    writer.text(&declaration.returned);
    writer.list(&declaration.returned_arguments, |writer, argument| {
        writer.text(argument);
    });
    writer.number(match declaration.inference {
        NullInference::Overridable => INFERENCE_OVERRIDABLE,
        NullInference::Closed => INFERENCE_CLOSED,
        NullInference::ClosedReturnsNull => INFERENCE_RETURNS_NULL,
    });
    writer.number(match declaration.return_type {
        ReturnType::Void => RETURN_TYPE_VOID,
        ReturnType::Primitive => RETURN_TYPE_PRIMITIVE,
        ReturnType::Reference => RETURN_TYPE_REFERENCE,
    });
    writer.list(&declaration.parameters, |writer, parameter| {
        writer.text(&parameter.name);
        write_position(writer, &parameter.position);
        writer.number(match parameter.nullness {
            ParameterNullness::Unannotated => NULLNESS_UNANNOTATED,
            ParameterNullness::Nullable => NULLNESS_NULLABLE,
            ParameterNullness::NonNull => NULLNESS_NON_NULL,
            ParameterNullness::NullableVariable => NULLNESS_NULLABLE_VARIABLE,
        });
        writer.number(match parameter.dereferenced {
            Dereferenced::No => DEREFERENCED_NO,
            Dereferenced::Always => DEREFERENCED_ALWAYS,
            Dereferenced::AlwaysFirst => DEREFERENCED_FIRST,
        });
        write_flag(writer, parameter.reading == Reading::Read);
        writer.text(&parameter.type_name);
        write_flag(writer, parameter.type_variable);
        write_flag(writer, parameter.accepts_null);
        write_type_arguments(writer, &parameter.type_arguments);
    });
    writer.list(&declaration.annotations, |writer, name| writer.text(name));
    write_owner(writer, &declaration.owner);
    writer.list(&declaration.fail_clauses, |writer, clause| {
        writer.list(clause, |writer, constraint| {
            write_constraint(writer, constraint.as_ref());
        });
    });
}

pub(super) fn write_owner(writer: &mut Writer, owner: &Owner) {
    writer.number(match owner.kind {
        OwnerKind::Class => OWNER_CLASS,
        OwnerKind::Interface => OWNER_INTERFACE,
        OwnerKind::Enum => OWNER_ENUM,
        OwnerKind::Record => OWNER_RECORD,
        OwnerKind::Annotation => OWNER_ANNOTATION,
        OwnerKind::Local => OWNER_LOCAL,
    });
    writer.text(&owner.name);
    writer.list(&owner.annotations, |writer, name| writer.text(name));
    write_marking(writer, owner.null_marking);
}

pub(super) fn write_marking(writer: &mut Writer, marking: NullMarking) {
    writer.number(match marking {
        NullMarking::Unspecified => MARKING_UNSPECIFIED,
        NullMarking::Marked => MARKING_MARKED,
        NullMarking::Unmarked => MARKING_UNMARKED,
    });
}

pub(super) fn read_marking(reader: &mut Reader<'_>) -> Result<NullMarking, Corrupt> {
    Ok(match reader.number()? {
        MARKING_UNSPECIFIED => NullMarking::Unspecified,
        MARKING_MARKED => NullMarking::Marked,
        MARKING_UNMARKED => NullMarking::Unmarked,
        _ => return Err(Corrupt),
    })
}

pub(super) fn read_declaration(reader: &mut Reader<'_>) -> Result<Declaration, Corrupt> {
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
        return_nullness: if read_flag(reader)? {
            ReturnNullness::Nullable
        } else {
            ReturnNullness::Unannotated
        },
        returns: match reader.number()? {
            RETURNS_NOTHING => Returns::Nothing,
            RETURNS_ALWAYS => Returns::Always(reader.text()?),
            RETURNS_VARIED => Returns::Varied,
            _ => return Err(Corrupt),
        },
        returned: reader.text()?,
        returned_arguments: reader.list(Reader::text)?,
        inference: match reader.number()? {
            INFERENCE_OVERRIDABLE => NullInference::Overridable,
            INFERENCE_CLOSED => NullInference::Closed,
            INFERENCE_RETURNS_NULL => NullInference::ClosedReturnsNull,
            _ => return Err(Corrupt),
        },
        return_type: match reader.number()? {
            RETURN_TYPE_VOID => ReturnType::Void,
            RETURN_TYPE_PRIMITIVE => ReturnType::Primitive,
            RETURN_TYPE_REFERENCE => ReturnType::Reference,
            _ => return Err(Corrupt),
        },
        parameters: reader.list(|reader| {
            Ok(Parameter {
                name: reader.text()?,
                position: read_position(reader)?,
                nullness: match reader.number()? {
                    NULLNESS_UNANNOTATED => ParameterNullness::Unannotated,
                    NULLNESS_NULLABLE => ParameterNullness::Nullable,
                    NULLNESS_NON_NULL => ParameterNullness::NonNull,
                    NULLNESS_NULLABLE_VARIABLE => ParameterNullness::NullableVariable,
                    _ => return Err(Corrupt),
                },
                dereferenced: match reader.number()? {
                    DEREFERENCED_NO => Dereferenced::No,
                    DEREFERENCED_ALWAYS => Dereferenced::Always,
                    DEREFERENCED_FIRST => Dereferenced::AlwaysFirst,
                    _ => return Err(Corrupt),
                },
                reading: if read_flag(reader)? {
                    Reading::Read
                } else {
                    Reading::Unread
                },
                type_name: reader.text()?,
                type_variable: read_flag(reader)?,
                accepts_null: read_flag(reader)?,
                type_arguments: read_type_arguments(reader)?,
            })
        })?,
        annotations: reader.list(Reader::text)?,
        owner: read_owner(reader)?,
        fail_clauses: reader.list(|reader| reader.list(read_constraint))?,
    })
}

pub(super) fn read_owner(reader: &mut Reader<'_>) -> Result<Owner, Corrupt> {
    Ok(Owner {
        kind: match reader.number()? {
            OWNER_CLASS => OwnerKind::Class,
            OWNER_INTERFACE => OwnerKind::Interface,
            OWNER_ENUM => OwnerKind::Enum,
            OWNER_RECORD => OwnerKind::Record,
            OWNER_ANNOTATION => OwnerKind::Annotation,
            OWNER_LOCAL => OwnerKind::Local,
            _ => return Err(Corrupt),
        },
        name: reader.text()?,
        annotations: reader.list(Reader::text)?,
        null_marking: read_marking(reader)?,
    })
}
