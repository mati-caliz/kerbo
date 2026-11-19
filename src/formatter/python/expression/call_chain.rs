use crate::formatter::python::ast::{Expr, NodeId};
use crate::formatter::python::context::Formatter;

const FLUENT_ATTRIBUTES_AFTER_PARENTHESES: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeState {
    CallLikePreceding(u32),
    FirstCallLike,
    BeforeFirstCallLike,
}

// Cómo se escribe una cadena `a.b().c()`: en estilo fluido cada `.método()` va en su línea, y sólo
// vale adentro de paréntesis y con dos atributos o más después de una llamada o un subíndice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallChainLayout {
    Default,
    Fluent(AttributeState),
    NonFluent,
}

fn is_call_or_subscript(f: &Formatter<'_>, node: NodeId) -> bool {
    matches!(f.ast.expr(node), Some(Expr::Call { .. } | Expr::Subscript { .. }))
}

impl CallChainLayout {
    #[must_use]
    pub const fn decrement_call_like_count(self) -> Self {
        match self {
            Self::Fluent(AttributeState::CallLikePreceding(count)) => {
                if count > 1 {
                    Self::Fluent(AttributeState::CallLikePreceding(count.saturating_sub(1)))
                } else {
                    Self::Fluent(AttributeState::FirstCallLike)
                }
            }
            Self::Default
            | Self::NonFluent
            | Self::Fluent(AttributeState::FirstCallLike | AttributeState::BeforeFirstCallLike) => self,
        }
    }

    #[must_use]
    pub const fn transition_after_attribute(self) -> Self {
        match self {
            Self::Fluent(AttributeState::FirstCallLike) => Self::Fluent(AttributeState::BeforeFirstCallLike),
            Self::Default
            | Self::NonFluent
            | Self::Fluent(AttributeState::CallLikePreceding(_) | AttributeState::BeforeFirstCallLike) => {
                self
            }
        }
    }

    pub fn from_expression(f: &Formatter<'_>, expression: NodeId) -> Self {
        let mut attributes_after_parentheses = 0_u32;
        let mut call_like_count = 0_u32;
        let mut root_value_parenthesized = false;
        let mut current = expression;
        loop {
            match f.ast.expr(current) {
                Some(Expr::Attribute { value, .. }) if f.is_expression_parenthesized(*value) => {
                    root_value_parenthesized = true;
                    break;
                }
                Some(Expr::Attribute { value, .. }) => {
                    let follows_a_call = u32::from(is_call_or_subscript(f, *value));
                    attributes_after_parentheses =
                        attributes_after_parentheses.saturating_add(follows_a_call);
                    current = *value;
                }
                Some(Expr::Call { func: inner, .. } | Expr::Subscript { value: inner, .. })
                    if !f.is_expression_parenthesized(*inner) =>
                {
                    let starts_a_call_chain = u32::from(!is_call_or_subscript(f, *inner));
                    call_like_count = call_like_count.saturating_add(starts_a_call_chain);
                    current = *inner;
                }
                Some(_) | None => break,
            }
        }
        let root = u32::from(root_value_parenthesized);
        if attributes_after_parentheses.saturating_add(root) < FLUENT_ATTRIBUTES_AFTER_PARENTHESES {
            Self::NonFluent
        } else {
            Self::Fluent(AttributeState::CallLikePreceding(
                call_like_count.saturating_add(root),
            ))
        }
    }

    // El primer nodo de la cadena decide el estilo; los de adentro reciben el que ya se decidió.
    pub fn apply_in_node(self, f: &Formatter<'_>, expression: NodeId) -> Self {
        match self {
            Self::Default => {
                if f.node_level.is_parenthesized() {
                    Self::from_expression(f, expression)
                } else {
                    Self::NonFluent
                }
            }
            Self::Fluent(_) | Self::NonFluent => self,
        }
    }

    pub const fn is_fluent(self) -> bool {
        matches!(self, Self::Fluent(_))
    }
}
