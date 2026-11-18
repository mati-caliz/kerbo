use std::collections::HashMap;
use std::rc::Rc;

use super::class_code::{ArgumentType, RawCode, method_shape, type_width};
use super::class_file::Cursor;
use super::code_pool::CodePool;
use super::opcodes::{
    GETFIELD, GETSTATIC, GOTO, GOTO_W, IFEQ, IFLE, IFNE, IFNONNULL, IFNULL, IINC, INVOKEDYNAMIC,
    INVOKEINTERFACE, INVOKESPECIAL, INVOKESTATIC, INVOKEVIRTUAL, LOOKUPSWITCH, PUTFIELD, PUTSTATIC,
    TABLESWITCH, WIDE,
};
use super::plain_instructions::{local_access, plain_operation};

const SWITCH_ALIGNMENT: usize = 4;
const INTERFACE_PADDING: usize = 2;
const WIDE_INCREMENT_SIZE: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallKind {
    Virtual,
    Special,
    Static,
    Interface,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct CallTarget {
    pub(super) owner: String,
    pub(super) name: String,
    pub(super) descriptor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Call {
    pub(super) kind: CallKind,
    pub(super) target: Rc<CallTarget>,
    pub(super) arguments: Vec<ArgumentType>,
    pub(super) returned: usize,
}

// Una instrucción reducida a lo que mueve en la pila y en las locales, con los saltos ya resueltos
// a la posición de la instrucción de destino.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Operation {
    Effect {
        pops: usize,
        pushes: usize,
    },
    LoadReference(usize),
    StoreReference(usize),
    StorePrimitive {
        local: usize,
        width: usize,
    },
    Duplicate {
        count: usize,
        under: usize,
    },
    Swap,
    Dereference {
        depth: usize,
        pops: usize,
        pushes: usize,
    },
    TypeCheck,
    Invoke(Call),
    NullBranch {
        jumps_on_null: bool,
        target: usize,
    },
    ZeroBranch {
        jumps_on_zero: bool,
        target: usize,
    },
    Branch {
        pops: usize,
        target: usize,
    },
    Jump(usize),
    Switch(Vec<usize>),
    Return,
    Throw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Handler {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) target: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Body {
    pub(super) max_locals: usize,
    pub(super) operations: Vec<Operation>,
    pub(super) handlers: Vec<Handler>,
}

// `None` si el método trae algo que el análisis no sigue (`jsr`/`ret`) o está mal formado.
pub(super) fn decode(pool: &CodePool, code: &RawCode) -> Option<Body> {
    let mut cursor = Cursor {
        bytes: &code.bytes,
        offset: 0,
    };
    let mut offsets = Vec::new();
    let mut operations = Vec::new();
    while cursor.offset < code.bytes.len() {
        let offset = cursor.offset;
        let opcode = cursor.u8().ok()?;
        offsets.push(offset);
        operations.push(operation(pool, &mut cursor, offset, opcode)?);
    }
    let positions: HashMap<usize, usize> = offsets
        .iter()
        .enumerate()
        .map(|(index, offset)| (*offset, index))
        .collect();
    for decoded in &mut operations {
        retarget(decoded, &positions)?;
    }
    let handlers = code
        .handlers
        .iter()
        .map(|handler| {
            Some(Handler {
                start: *positions.get(&handler.start)?,
                end: positions.get(&handler.end).copied().unwrap_or(operations.len()),
                target: *positions.get(&handler.target)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Body {
        max_locals: code.max_locals,
        operations,
        handlers,
    })
}

fn retarget(operation: &mut Operation, positions: &HashMap<usize, usize>) -> Option<()> {
    match operation {
        Operation::NullBranch { target, .. }
        | Operation::ZeroBranch { target, .. }
        | Operation::Branch { target, .. }
        | Operation::Jump(target) => *target = *positions.get(target)?,
        Operation::Switch(targets) => {
            for target in targets {
                *target = *positions.get(target)?;
            }
        }
        Operation::Effect { .. }
        | Operation::LoadReference(_)
        | Operation::StoreReference(_)
        | Operation::StorePrimitive { .. }
        | Operation::Duplicate { .. }
        | Operation::Swap
        | Operation::Dereference { .. }
        | Operation::TypeCheck
        | Operation::Invoke(_)
        | Operation::Return
        | Operation::Throw => {}
    }
    Some(())
}

fn operation(pool: &CodePool, cursor: &mut Cursor<'_>, offset: usize, opcode: u8) -> Option<Operation> {
    match opcode {
        WIDE => wide(cursor),
        TABLESWITCH | LOOKUPSWITCH => switch(cursor, offset, opcode),
        IFEQ..=GOTO | IFNULL..=GOTO_W => jump(cursor, offset, opcode),
        GETSTATIC..=PUTFIELD => field(pool, cursor, opcode),
        INVOKEVIRTUAL..=INVOKEDYNAMIC => invoke(pool, cursor, opcode),
        _ => plain_operation(cursor, opcode),
    }
}

fn wide(cursor: &mut Cursor<'_>) -> Option<Operation> {
    let opcode = cursor.u8().ok()?;
    let local = usize::from(cursor.u16().ok()?);
    if opcode == IINC {
        cursor.take(WIDE_INCREMENT_SIZE).ok()?;
        return Some(Operation::Effect { pops: 0, pushes: 0 });
    }
    local_access(opcode, local)
}

fn relative(offset: usize, delta: i32) -> Option<usize> {
    usize::try_from(i64::try_from(offset).ok()?.checked_add(i64::from(delta))?).ok()
}

fn signed(cursor: &mut Cursor<'_>) -> Option<i32> {
    Some(i32::from_be_bytes(cursor.u32().ok()?.to_be_bytes()))
}

fn jump(cursor: &mut Cursor<'_>, offset: usize, opcode: u8) -> Option<Operation> {
    let delta = if opcode == GOTO_W {
        signed(cursor)?
    } else {
        i32::from(i16::from_be_bytes(cursor.u16().ok()?.to_be_bytes()))
    };
    let target = relative(offset, delta)?;
    Some(match opcode {
        IFEQ | IFNE => Operation::ZeroBranch {
            jumps_on_zero: opcode == IFEQ,
            target,
        },
        IFNULL | IFNONNULL => Operation::NullBranch {
            jumps_on_null: opcode == IFNULL,
            target,
        },
        GOTO | GOTO_W => Operation::Jump(target),
        _ if opcode <= IFLE => Operation::Branch { pops: 1, target },
        _ if opcode < GOTO => Operation::Branch { pops: 2, target },
        _ => return None,
    })
}

fn switch(cursor: &mut Cursor<'_>, offset: usize, opcode: u8) -> Option<Operation> {
    let padding = (SWITCH_ALIGNMENT - cursor.offset % SWITCH_ALIGNMENT) % SWITCH_ALIGNMENT;
    cursor.take(padding).ok()?;
    let mut targets = vec![relative(offset, signed(cursor)?)?];
    if opcode == TABLESWITCH {
        let low = i64::from(signed(cursor)?);
        let high = i64::from(signed(cursor)?);
        for _ in low..=high {
            targets.push(relative(offset, signed(cursor)?)?);
        }
    } else {
        let pairs = signed(cursor)?;
        for _ in 0..pairs {
            signed(cursor)?;
            targets.push(relative(offset, signed(cursor)?)?);
        }
    }
    Some(Operation::Switch(targets))
}

fn field(pool: &CodePool, cursor: &mut Cursor<'_>, opcode: u8) -> Option<Operation> {
    let width = type_width(pool.member(cursor.u16().ok()?)?.descriptor);
    Some(match opcode {
        GETSTATIC => Operation::Effect {
            pops: 0,
            pushes: width,
        },
        PUTSTATIC => Operation::Effect {
            pops: width,
            pushes: 0,
        },
        GETFIELD => Operation::Dereference {
            depth: 0,
            pops: 1,
            pushes: width,
        },
        _ => Operation::Dereference {
            depth: width,
            pops: width + 1,
            pushes: 0,
        },
    })
}

fn invoke(pool: &CodePool, cursor: &mut Cursor<'_>, opcode: u8) -> Option<Operation> {
    let index = cursor.u16().ok()?;
    let (kind, target) = if opcode == INVOKEDYNAMIC {
        cursor.take(INTERFACE_PADDING).ok()?;
        let target = CallTarget {
            owner: String::new(),
            name: String::new(),
            descriptor: pool.dynamic_descriptor(index)?.to_owned(),
        };
        (CallKind::Dynamic, target)
    } else {
        let member = pool.member(index)?;
        let target = CallTarget {
            owner: member.owner.to_owned(),
            name: member.name.to_owned(),
            descriptor: member.descriptor.to_owned(),
        };
        (call_kind(cursor, opcode)?, target)
    };
    let shape = method_shape(&target.descriptor)?;
    Some(Operation::Invoke(Call {
        kind,
        target: Rc::new(target),
        arguments: shape.arguments,
        returned: shape.returned,
    }))
}

fn call_kind(cursor: &mut Cursor<'_>, opcode: u8) -> Option<CallKind> {
    Some(match opcode {
        INVOKEVIRTUAL => CallKind::Virtual,
        INVOKESPECIAL => CallKind::Special,
        INVOKESTATIC => CallKind::Static,
        INVOKEINTERFACE => {
            cursor.take(INTERFACE_PADDING).ok()?;
            CallKind::Interface
        }
        _ => return None,
    })
}
