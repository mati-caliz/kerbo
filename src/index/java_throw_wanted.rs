use super::java::Declaration;
use super::java_throw_lookup::accepts;
use crate::jvm::method::Method;
use crate::lint::java_throws::{ArgumentKind, CallStep};

const TEXT_PARAMETERS: &[&str] = &[
    "Ljava/lang/String;",
    "Ljava/lang/Object;",
    "Ljava/lang/CharSequence;",
    "Ljava/lang/Comparable;",
    "Ljava/io/Serializable;",
    "Ljava/lang/constant/Constable;",
    "Ljava/lang/constant/ConstantDesc;",
];
const CHARSET_PARAMETERS: &[&str] = &[
    "Ljava/nio/charset/Charset;",
    "Ljava/lang/Object;",
    "Ljava/lang/Comparable;",
];

// Sin `arity` es una referencia a método (`Tipo::metodo`): sirve cualquier sobrecarga.
#[derive(Clone, Copy)]
pub(super) struct Wanted<'call> {
    pub name: &'call str,
    pub arity: Option<usize>,
    pub kinds: &'call [ArgumentKind],
}

impl<'call> Wanted<'call> {
    pub const fn by_arity(name: &'call str, arity: usize) -> Self {
        Self {
            name,
            arity: Some(arity),
            kinds: &[],
        }
    }

    pub const fn any_arity(name: &'call str) -> Self {
        Self {
            name,
            arity: None,
            kinds: &[],
        }
    }

    pub const fn admits(&self, declaration: &Declaration) -> bool {
        match self.arity {
            Some(arity) => accepts(declaration, arity),
            None => true,
        }
    }

    pub fn admits_method(&self, method: &Method) -> bool {
        self.arity.is_none_or(|arity| method.accepts(arity))
    }

    pub fn of_step(step: &'call CallStep) -> Self {
        Self {
            name: &step.name,
            arity: Some(step.arity),
            kinds: &step.arguments,
        }
    }

    pub fn rules_out(&self, method: &Method) -> bool {
        !method.is_varargs()
            && method.parameter_count() == self.kinds.len()
            && self.kinds.iter().enumerate().any(|(position, kind)| match kind {
                ArgumentKind::Unknown | ArgumentKind::Lambda(_) | ArgumentKind::Reference(..) => false,
                ArgumentKind::Text => method.rejects_argument(position, TEXT_PARAMETERS),
                ArgumentKind::Charset => method.rejects_argument(position, CHARSET_PARAMETERS),
            })
    }
}
