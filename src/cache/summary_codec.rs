use super::codec::{Corrupt, Reader, Writer};
use crate::index::java::{CallSite, Declaration, JavaSummary, Parameter, Position};
use crate::lint::java_calls::{Arity, Body, ReturnType, Returns, Visibility};
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_constants::Constant;
use crate::lint::java_null_flow::{Dereference, NullFlow};
use crate::lint::java_owner::{Owner, OwnerKind};
use crate::lint::java_receivers::Receiver;
use crate::lint::java_resources::ResourceCandidate;

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
const OWNER_CLASS: u64 = 0;
const OWNER_INTERFACE: u64 = 1;
const OWNER_ENUM: u64 = 2;
const OWNER_RECORD: u64 = 3;
const OWNER_ANNOTATION: u64 = 4;
const OWNER_LOCAL: u64 = 5;
const RETURN_TYPE_VOID: u64 = 0;
const RETURN_TYPE_PRIMITIVE: u64 = 1;
const RETURN_TYPE_REFERENCE: u64 = 2;
const ROOT_INSTANCE: u64 = 0;
const ROOT_CLASS: u64 = 1;
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
        write_flag(writer, call.result_used);
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
        write_chain(writer, &candidate.origin);
        writer.size(candidate.line);
        writer.size(candidate.column);
    });
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
                receiver: read_receiver(reader)?,
                arguments: reader.list(read_optional_text)?,
                result_used: read_flag(reader)?,
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
                origin: read_chain(reader)?,
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
    writer.text(&declaration.returned);
    writer.number(match declaration.return_type {
        ReturnType::Void => RETURN_TYPE_VOID,
        ReturnType::Primitive => RETURN_TYPE_PRIMITIVE,
        ReturnType::Reference => RETURN_TYPE_REFERENCE,
    });
    writer.list(&declaration.parameters, |writer, parameter| {
        writer.text(&parameter.name);
        write_position(writer, &parameter.position);
        write_flag(writer, parameter.declares_nullness);
        write_flag(writer, parameter.dereferenced);
    });
    writer.list(&declaration.annotations, |writer, name| writer.text(name));
    writer.number(match declaration.owner.kind {
        OwnerKind::Class => OWNER_CLASS,
        OwnerKind::Interface => OWNER_INTERFACE,
        OwnerKind::Enum => OWNER_ENUM,
        OwnerKind::Record => OWNER_RECORD,
        OwnerKind::Annotation => OWNER_ANNOTATION,
        OwnerKind::Local => OWNER_LOCAL,
    });
    writer.text(&declaration.owner.name);
    writer.list(&declaration.owner.annotations, |writer, name| writer.text(name));
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
        returned: reader.text()?,
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
                declares_nullness: read_flag(reader)?,
                dereferenced: read_flag(reader)?,
            })
        })?,
        annotations: reader.list(Reader::text)?,
        owner: Owner {
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
        },
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

fn write_receiver(writer: &mut Writer, receiver: &Receiver) {
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

fn read_receiver(reader: &mut Reader<'_>) -> Result<Receiver, Corrupt> {
    match reader.number()? {
        RECEIVER_UNKNOWN => Ok(Receiver::Unknown),
        RECEIVER_TYPE => Ok(Receiver::Type(reader.text()?)),
        RECEIVER_SCOPE => Ok(Receiver::Scope(reader.list(Reader::text)?)),
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

fn write_chain(writer: &mut Writer, chain: &Chain) {
    match &chain.root {
        ChainRoot::Instance(name) => {
            writer.number(ROOT_INSTANCE);
            writer.text(name);
        }
        ChainRoot::Class(name) => {
            writer.number(ROOT_CLASS);
            writer.text(name);
        }
    }
    writer.list(&chain.steps, |writer, step| {
        writer.text(&step.name);
        writer.size(step.arity);
        writer.size(step.line);
        writer.size(step.column);
        write_flag(writer, step.checked);
    });
}

fn read_chain(reader: &mut Reader<'_>) -> Result<Chain, Corrupt> {
    Ok(Chain {
        root: match reader.number()? {
            ROOT_INSTANCE => ChainRoot::Instance(reader.text()?),
            ROOT_CLASS => ChainRoot::Class(reader.text()?),
            _ => return Err(Corrupt),
        },
        steps: reader.list(|reader| {
            Ok(ChainStep {
                name: reader.text()?,
                arity: reader.size()?,
                line: reader.size()?,
                column: reader.size()?,
                checked: read_flag(reader)?,
            })
        })?,
    })
}

fn write_flow(writer: &mut Writer, flow: &NullFlow) {
    write_chain(writer, &flow.origin);
    writer.list(&flow.dereferences, |writer, used| {
        writer.text(&used.name);
        writer.size(used.line);
        writer.size(used.column);
    });
}

fn read_flow(reader: &mut Reader<'_>) -> Result<NullFlow, Corrupt> {
    Ok(NullFlow {
        origin: read_chain(reader)?,
        dereferences: reader.list(|reader| {
            Ok(Dereference {
                name: reader.text()?,
                line: reader.size()?,
                column: reader.size()?,
            })
        })?,
    })
}
