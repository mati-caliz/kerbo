use tree_sitter::Node;

use super::{NumericValues, numbers};
use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::syntax::{TEST, is_double_bracket, named_children, text};
use crate::shell::test_expression::{Condition, parse};
use crate::shell::word_pieces::{
    PieceKind, has_glob, literal_string, pieces, simplified_text, simplified_text_is_number,
};

const BINARY_OPERATORS: &[&str] = &[
    "-nt", "-ot", "-ef", "==", "!=", "<=", ">=", "-eq", "-ne", "-lt", "-le", "-gt", "-ge", "=~", ">", "<",
    "=",
];
const UNARY_OPERATORS: &[&str] = &[
    "!", "-a", "-b", "-c", "-d", "-e", "-f", "-g", "-h", "-L", "-k", "-p", "-r", "-s", "-S", "-t", "-u",
    "-w", "-x", "-O", "-G", "-N", "-z", "-n", "-o", "-v", "-R",
];
const EQUALITY_OPERATORS: &[&str] = &["=", "==", "!="];
const ASSIGNMENT_EQUALITY_OPERATORS: &[&str] = &["=", "=="];
const NOT_EQUAL_OPERATORS: &[&str] = &["-ne", "!="];
const NUMERIC_EQUAL: &str = "-eq";
const REGEX_MATCH: &str = "=~";
const LOGICAL_AND: &str = "&&";
const LOGICAL_OR: &str = "||";
const FLAG_AND: &str = "-a";
const FLAG_OR: &str = "-o";
const LEGACY_EXISTS: &str = "-a";
const GLOB_START: char = '*';
const NOT_GLOB_LEAD: &[char] = &['\\', '.'];

pub fn check(
    node: Node<'_>,
    context: &Context<'_, '_>,
    numeric_values: &NumericValues,
    findings: &mut Findings,
) {
    if node.kind() != TEST {
        return;
    }
    let source = context.source;
    let single_bracket = !is_double_bracket(node, source);
    if single_bracket && named_children(node).is_empty() {
        findings.report(ShellRule::Sc2212, node, text(node, source));
        return;
    }
    let Some(condition) = parse(node, source) else {
        return;
    };
    let mut checker = Checker {
        single_bracket,
        context,
        numeric_values,
        findings,
    };
    condition.visit(true, &mut |part, _| match part {
        Condition::And(operator, left, right) => checker.conjunction(*operator, left, right),
        Condition::Or(operator, left, right) => checker.disjunction(*operator, left, right),
        Condition::Unary(operator, _) => checker.unary(*operator),
        Condition::Binary(operator, left, right) => checker.binary(*operator, *left, *right),
        Condition::Not(bang, inner) => checker.negation(*bang, inner),
        Condition::Group(_) | Condition::Nullary(_) => {}
    });
}

struct Checker<'run, 'tree, 'source> {
    single_bracket: bool,
    context: &'run Context<'tree, 'source>,
    numeric_values: &'run NumericValues,
    findings: &'run mut Findings,
}

impl Checker<'_, '_, '_> {
    fn report(&mut self, rule: ShellRule, node: Node<'_>) {
        self.findings.report(rule, node, text(node, self.context.source));
    }

    // SC2107, SC2108 y SC2333: `checkConditionalAndOrs` y `checkAndEq`.
    fn conjunction(&mut self, operator: Node<'_>, left: &Condition<'_>, right: &Condition<'_>) {
        let source = self.context.source;
        let operator_text = text(operator, source);
        if self.single_bracket && operator_text == LOGICAL_AND {
            self.report(ShellRule::Sc2107, operator);
        } else if !self.single_bracket && operator_text == FLAG_AND {
            self.report(ShellRule::Sc2108, operator);
        }
        if let Some(repeated) = repeated_subject(left, right, source)
            && equality_with_literals(&repeated, source)
        {
            self.report(ShellRule::Sc2333, operator);
        }
    }

    // SC2109, SC2110 y SC2055: `checkConditionalAndOrs` y `checkOrNeq`.
    fn disjunction(&mut self, operator: Node<'_>, left: &Condition<'_>, right: &Condition<'_>) {
        let source = self.context.source;
        let operator_text = text(operator, source);
        if self.single_bracket && operator_text == LOGICAL_OR {
            self.report(ShellRule::Sc2109, operator);
        } else if !self.single_bracket && operator_text == FLAG_OR {
            self.report(ShellRule::Sc2110, operator);
        }
        if let Some(repeated) = repeated_subject(left, right, source)
            && NOT_EQUAL_OPERATORS.contains(&repeated.operator)
            && !has_glob(&pieces(repeated.first, source))
            && !has_glob(&pieces(repeated.second, source))
        {
            self.report(ShellRule::Sc2055, operator);
        }
    }

    // SC2332: `checkNegatedUnaryOps`.
    fn negation(&mut self, bang: Node<'_>, inner: &Condition<'_>) {
        if self.single_bracket
            && self.context.dialect == Dialect::Bash
            && let Condition::Unary(operator, _) = inner
            && [FLAG_AND, FLAG_OR].contains(&text(*operator, self.context.source))
        {
            self.report(ShellRule::Sc2332, bang);
        }
    }

    // SC2058 y SC2331: `checkValidCondOps` y `checkUnaryTestA`.
    fn unary(&mut self, operator: Node<'_>) {
        let operator_text = text(operator, self.context.source);
        if !UNARY_OPERATORS.contains(&operator_text) {
            self.report(ShellRule::Sc2058, operator);
        }
        if operator_text == LEGACY_EXISTS {
            self.report(ShellRule::Sc2331, operator);
        }
    }

    fn binary(&mut self, operator: Node<'_>, left: Node<'_>, right: Node<'_>) {
        let operator_text = text(operator, self.context.source);
        if !BINARY_OPERATORS.contains(&operator_text) {
            self.report(ShellRule::Sc2057, operator);
        }
        numbers::check(
            numbers::Comparison {
                operator,
                left,
                right,
                single_bracket: self.single_bracket,
            },
            self.context,
            self.numeric_values,
            self.findings,
        );
        if operator_text == REGEX_MATCH {
            self.regex(operator, right);
        }
        if EQUALITY_OPERATORS.contains(&operator_text) {
            self.glob_comparison(right);
        }
    }

    // SC2074 y SC2049: `checkSingleBracketOperators` y `checkGlobbedRegex`.
    fn regex(&mut self, operator: Node<'_>, right: Node<'_>) {
        if self.single_bracket {
            if matches!(self.context.dialect, Dialect::Bash | Dialect::Ksh) {
                self.report(ShellRule::Sc2074, operator);
            }
        } else if is_confused_glob_regex(&simplified_text(&pieces(right, self.context.source))) {
            self.report(ShellRule::Sc2049, right);
        }
    }

    // SC2053, SC2081 y SC2330: `checkComparisonAgainstGlob`.
    fn glob_comparison(&mut self, right: Node<'_>) {
        let right_pieces = pieces(right, self.context.source);
        if self.single_bracket {
            if has_glob(&right_pieces) {
                self.report(ShellRule::Sc2081, right);
            }
            return;
        }
        if matches!(right_pieces.as_slice(), [only] if only.kind == PieceKind::Braced) {
            self.report(ShellRule::Sc2053, right);
        } else if self.context.dialect == Dialect::Busybox && has_glob(&right_pieces) {
            self.report(ShellRule::Sc2330, right);
        }
    }
}

struct RepeatedSubject<'tree, 'source> {
    operator: &'source str,
    first: Node<'tree>,
    second: Node<'tree>,
}

fn repeated_subject<'tree, 'source>(
    left: &Condition<'tree>,
    right: &Condition<'tree>,
    source: &'source [u8],
) -> Option<RepeatedSubject<'tree, 'source>> {
    let (
        Condition::Binary(left_operator, left_subject, first),
        Condition::Binary(right_operator, right_subject, second),
    ) = (left, right)
    else {
        return None;
    };
    let operator = text(*left_operator, source);
    (operator == text(*right_operator, source)
        && text(*left_subject, source) == text(*right_subject, source)
        && text(*first, source) != text(*second, source))
    .then_some(RepeatedSubject {
        operator,
        first: *first,
        second: *second,
    })
}

// `checkAndEqOperands`.
fn equality_with_literals(repeated: &RepeatedSubject<'_, '_>, source: &[u8]) -> bool {
    operands_exclude_each_other(repeated.operator, repeated.first, repeated.second, source)
}

pub fn operands_exclude_each_other(operator: &str, first: Node<'_>, second: Node<'_>, source: &[u8]) -> bool {
    if operator == NUMERIC_EQUAL {
        return is_literal_number(first, source) && is_literal_number(second, source);
    }
    ASSIGNMENT_EQUALITY_OPERATORS.contains(&operator)
        && literal_string(&pieces(first, source)).is_some()
        && literal_string(&pieces(second, source)).is_some()
}

pub fn is_literal_number(word: Node<'_>, source: &[u8]) -> bool {
    literal_string(&pieces(word, source)).is_some_and(|literal| simplified_text_is_number(&literal))
}

// `isConfusedGlobRegex`.
fn is_confused_glob_regex(regex: &str) -> bool {
    if regex.starts_with(GLOB_START) {
        return true;
    }
    let mut characters = regex.chars();
    matches!(
        (characters.next(), characters.next(), characters.next()),
        (Some(lead), Some(GLOB_START), None) if !NOT_GLOB_LEAD.contains(&lead)
    )
}
