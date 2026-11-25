use super::is_pattern_parenthesized;
use crate::formatter::python::ast::{Expr, NodeId, Other};
use crate::formatter::python::context::Formatter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
enum Precedence {
    #[default]
    None,
    Additive,
    Or,
}

#[derive(Debug, Clone, Copy, Default)]
enum First {
    #[default]
    None,
    Token,
    Pattern(NodeId),
}

#[derive(Debug, Default)]
struct Visitor {
    max_precedence: Precedence,
    max_precedence_count: usize,
    any_parenthesized_expressions: bool,
    last: Option<NodeId>,
    first: First,
}

impl Visitor {
    fn visit_pattern(&mut self, f: &Formatter<'_>, pattern: NodeId) {
        match f.ast.other(pattern) {
            Some(Other::PatternSequence { .. } | Other::PatternMapping { .. }) => {
                self.any_parenthesized_expressions = true;
            }
            Some(Other::PatternValue { value }) => {
                if matches!(f.ast.expr(*value), Some(Expr::BinOp { .. })) {
                    self.update_max_precedence(Precedence::Additive, 1);
                }
            }
            Some(Other::PatternClass { .. }) => {
                self.any_parenthesized_expressions = true;
                self.set_first_if_none(First::Token);
            }
            Some(Other::PatternAs {
                pattern: Some(inner), ..
            }) => self.visit_sub_pattern(f, *inner),
            Some(Other::PatternOr { patterns }) => {
                self.update_max_precedence(Precedence::Or, patterns.len().saturating_sub(1));
                for inner in patterns {
                    self.visit_sub_pattern(f, *inner);
                }
            }
            Some(_) | None => {}
        }
    }

    fn visit_sub_pattern(&mut self, f: &Formatter<'_>, pattern: NodeId) {
        self.last = Some(pattern);
        if is_pattern_parenthesized(f, pattern) {
            self.any_parenthesized_expressions = true;
        } else {
            self.visit_pattern(f, pattern);
        }
        self.set_first_if_none(First::Pattern(pattern));
    }

    const fn set_first_if_none(&mut self, first: First) {
        if matches!(self.first, First::None) {
            self.first = first;
        }
    }

    fn update_max_precedence(&mut self, precedence: Precedence, count: usize) {
        match self.max_precedence.cmp(&precedence) {
            std::cmp::Ordering::Less => {
                self.max_precedence_count = count;
                self.max_precedence = precedence;
            }
            std::cmp::Ordering::Equal => {
                self.max_precedence_count = self.max_precedence_count.saturating_add(count);
            }
            std::cmp::Ordering::Greater => {}
        }
    }
}

fn has_parentheses_and_is_non_empty(f: &Formatter<'_>, pattern: NodeId) -> bool {
    let has_own_non_empty = match f.ast.other(pattern) {
        Some(Other::PatternAs {
            pattern: Some(inner), ..
        }) => has_parentheses_and_is_non_empty(f, *inner),
        Some(Other::PatternSequence { patterns, .. } | Other::PatternMapping { patterns, .. }) => {
            !patterns.is_empty() || f.comments.has_dangling(pattern)
        }
        Some(Other::PatternClass { arguments, .. }) => matches!(
            f.ast.other(*arguments),
            Some(Other::PatternArguments { patterns, .. }) if !patterns.is_empty()
        ),
        Some(_) | None => false,
    };
    has_own_non_empty || is_pattern_parenthesized(f, pattern)
}

pub fn can_omit_optional_parentheses(f: &Formatter<'_>, pattern: NodeId) -> bool {
    let mut visitor = Visitor::default();
    visitor.visit_pattern(f, pattern);
    if !visitor.any_parenthesized_expressions || visitor.max_precedence_count > 1 {
        return false;
    }
    visitor
        .last
        .is_some_and(|last| has_parentheses_and_is_non_empty(f, last))
        || match visitor.first {
            First::Pattern(first) => has_parentheses_and_is_non_empty(f, first),
            First::None | First::Token => false,
        }
}
