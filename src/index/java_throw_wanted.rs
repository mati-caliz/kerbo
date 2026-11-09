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

#[derive(Clone, Copy)]
pub(super) struct Wanted<'call> {
    pub name: &'call str,
    pub arity: usize,
    pub kinds: &'call [ArgumentKind],
}

impl<'call> Wanted<'call> {
    pub const fn by_arity(name: &'call str, arity: usize) -> Self {
        Self {
            name,
            arity,
            kinds: &[],
        }
    }

    pub fn of_step(step: &'call CallStep) -> Self {
        Self {
            name: &step.name,
            arity: step.arity,
            kinds: &step.arguments,
        }
    }

    pub fn rules_out(&self, method: &Method) -> bool {
        !method.is_varargs()
            && method.parameter_count() == self.kinds.len()
            && self.kinds.iter().enumerate().any(|(position, kind)| match kind {
                ArgumentKind::Unknown => false,
                ArgumentKind::Text => method.rejects_argument(position, TEXT_PARAMETERS),
                ArgumentKind::Charset => method.rejects_argument(position, CHARSET_PARAMETERS),
            })
    }
}
