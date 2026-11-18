use super::class_file::Cursor;
use super::instructions::Operation;
use super::opcodes::{
    ACONST_NULL, ALOAD, ALOAD_3, ANEWARRAY, ARRAYLENGTH, ASTORE, ASTORE_3, ATHROW, BIPUSH, CHECKCAST, DCMPG,
    DCMPL, DCONST_0, DCONST_1, DNEG, DREM, DUP, DUP2_X2, FCMPG, FCMPL, FCONST_0, FCONST_2, I2L, I2S, IADD,
    IALOAD, IAND, IASTORE, ICONST_5, IINC, ILOAD, ILOAD_0, INEG, INSTANCEOF, IRETURN, ISHL, ISTORE, ISTORE_0,
    LCMP, LCONST_0, LCONST_1, LDC, LDC_W, LDC2_W, LUSHR, LXOR, MONITORENTER, MONITOREXIT, MULTIANEWARRAY,
    NEW, NEWARRAY, POP, POP2, RETURN, SALOAD, SASTORE, SIPUSH, SWAP,
};

const KIND_WIDTHS: [usize; 8] = [1, 2, 1, 2, 1, 1, 1, 1];
const NUMERIC_KINDS: u8 = 4;
const INTEGER_KINDS: u8 = 2;
const SHORT_LOCALS: u8 = 4;
const DUPLICATE_PLACES: u8 = 3;
const CLASS_INDEX_SIZE: usize = 2;
const CONVERSIONS: [(usize, usize); 15] = [
    (1, 2),
    (1, 1),
    (1, 2),
    (2, 1),
    (2, 1),
    (2, 2),
    (1, 1),
    (1, 2),
    (1, 2),
    (2, 1),
    (2, 2),
    (2, 1),
    (1, 1),
    (1, 1),
    (1, 1),
];

fn kind_width(kind: u8) -> usize {
    KIND_WIDTHS.get(usize::from(kind)).copied().unwrap_or(1)
}

// `iload 3`, `astore 2` y sus formas cortas: sólo importa si lo que pasa por la local es una
// referencia.
pub(super) fn local_access(opcode: u8, local: usize) -> Option<Operation> {
    Some(match opcode {
        ALOAD => Operation::LoadReference(local),
        ASTORE => Operation::StoreReference(local),
        ILOAD..ALOAD => Operation::Effect {
            pops: 0,
            pushes: kind_width(opcode - ILOAD),
        },
        ISTORE..ASTORE => Operation::StorePrimitive {
            local,
            width: kind_width(opcode - ISTORE),
        },
        _ => return None,
    })
}

fn short_local_access(base: u8, first_short: u8, opcode: u8) -> Option<Operation> {
    let index = opcode - first_short;
    local_access(base + index / SHORT_LOCALS, usize::from(index % SHORT_LOCALS))
}

pub(super) fn plain_operation(cursor: &mut Cursor<'_>, opcode: u8) -> Option<Operation> {
    match opcode {
        ILOAD..=ALOAD | ISTORE..=ASTORE => local_access(opcode, usize::from(cursor.u8().ok()?)),
        ILOAD_0..=ALOAD_3 => short_local_access(ILOAD, ILOAD_0, opcode),
        ISTORE_0..=ASTORE_3 => short_local_access(ISTORE, ISTORE_0, opcode),
        MULTIANEWARRAY => {
            cursor.take(CLASS_INDEX_SIZE).ok()?;
            Some(Operation::Effect {
                pops: usize::from(cursor.u8().ok()?),
                pushes: 1,
            })
        }
        _ => {
            cursor.take(operand_size(opcode)).ok()?;
            without_operands(opcode)
        }
    }
}

const fn operand_size(opcode: u8) -> usize {
    match opcode {
        BIPUSH | LDC | NEWARRAY => 1,
        SIPUSH | LDC_W | LDC2_W | IINC | NEW | ANEWARRAY | CHECKCAST | INSTANCEOF => 2,
        _ => 0,
    }
}

fn without_operands(opcode: u8) -> Option<Operation> {
    Some(match opcode {
        IALOAD..=SALOAD => Operation::Dereference {
            depth: 1,
            pops: 2,
            pushes: kind_width(opcode - IALOAD),
        },
        IASTORE..=SASTORE => {
            let width = kind_width(opcode - IASTORE);
            Operation::Dereference {
                depth: width + 1,
                pops: width + 2,
                pushes: 0,
            }
        }
        DUP..=DUP2_X2 => Operation::Duplicate {
            count: usize::from((opcode - DUP) / DUPLICATE_PLACES) + 1,
            under: usize::from((opcode - DUP) % DUPLICATE_PLACES),
        },
        SWAP => Operation::Swap,
        ARRAYLENGTH => Operation::Dereference {
            depth: 0,
            pops: 1,
            pushes: 1,
        },
        MONITORENTER => Operation::Dereference {
            depth: 0,
            pops: 1,
            pushes: 0,
        },
        INSTANCEOF => Operation::TypeCheck,
        IRETURN..=RETURN => Operation::Return,
        ATHROW => Operation::Throw,
        _ => return stack_effect(opcode).map(|(pops, pushes)| Operation::Effect { pops, pushes }),
    })
}

fn stack_effect(opcode: u8) -> Option<(usize, usize)> {
    Some(match opcode {
        ACONST_NULL..=ICONST_5 | FCONST_0..=FCONST_2 | BIPUSH..=LDC_W | NEW => (0, 1),
        LCONST_0..=LCONST_1 | DCONST_0..=DCONST_1 | LDC2_W => (0, 2),
        POP | MONITOREXIT => (1, 0),
        POP2 => (2, 0),
        NEWARRAY | ANEWARRAY => (1, 1),
        LCMP | DCMPL | DCMPG => (4, 1),
        FCMPL | FCMPG => (2, 1),
        IADD..=LXOR => arithmetic_effect(opcode),
        I2L..=I2S => *CONVERSIONS.get(usize::from(opcode - I2L))?,
        _ if opcode < ACONST_NULL || opcode == IINC || opcode == CHECKCAST => (0, 0),
        _ => return None,
    })
}

fn arithmetic_effect(opcode: u8) -> (usize, usize) {
    match opcode {
        IADD..=DREM => {
            let width = kind_width((opcode - IADD) % NUMERIC_KINDS);
            (width + width, width)
        }
        INEG..=DNEG => {
            let width = kind_width(opcode - INEG);
            (width, width)
        }
        ISHL..=LUSHR => {
            let width = kind_width((opcode - ISHL) % INTEGER_KINDS);
            (width + 1, width)
        }
        _ => {
            let width = kind_width((opcode - IAND) % INTEGER_KINDS);
            (width + width, width)
        }
    }
}
