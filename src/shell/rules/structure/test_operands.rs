use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings, Position};
use crate::shell::dialect::Dialect;
use crate::shell::pseudo_glob::{can_overlap, fuzzy};
use crate::shell::syntax::{
    BODY_FIELD, COMMAND, COMMAND_SUBSTITUTION, PIPELINE, REDIRECTED, STRING, TEST, is_backticked,
    is_double_bracket, named_children, text,
};
use crate::shell::test_expression::{Condition, parse};
use crate::shell::word_pieces::{
    Piece, PieceKind, flattened, has_array_expansion, has_brace_expansion, has_glob, is_constant,
    literal_string, pieces,
};

const ARITHMETIC_OPERATORS: &[&str] = &["-eq", "-ne", "-lt", "-le", "-gt", "-ge"];
const FILE_COMPARISONS: &[&str] = &["-nt", "-ot", "-ef"];
const EQUALITY_OPERATORS: &[&str] = &["=", "==", "!="];
const STRING_OPERATORS: &[&str] = &["=", "==", "!=", "=~"];
const REGEX_MATCH: &str = "=~";
const NON_EMPTY: &str = "-n";
const EMPTY: &str = "-z";
const VARIABLE_IS_SET: &str = "-v";
const KSH_FIRST_GLOB_OPERATORS: &[&str] = &[
    "-b", "-c", "-d", "-f", "-g", "-k", "-p", "-r", "-s", "-u", "-w", "-x", "-L", "-h", "-N", "-O", "-G",
    "-R", "-S",
];
const REGEX_METACHARACTERS: &[char] = &['[', ']', '*', '.', '+', '(', ')', '|'];
const EQUALS: char = '=';
const LEADING_X: char = 'x';
const TEST_COMMAND: &str = "test";
const FALSE_WORD: &str = "false";
const TRUE_WORD: &str = "true";
const ZERO: &str = "0";
const ONE: &str = "1";
const GREP_COMMANDS: &[&str] = &[
    "grep", "egrep", "fgrep", "bz3grep", "bzgrep", "xzgrep", "zgrep", "zipgrep", "zstdgrep",
];
const PATH_SEPARATOR: char = '/';

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() == TEST {
        check_test(node, context, findings);
    } else if node.kind() == COMMAND {
        check_test_command_leading_x(node, context.source, findings);
    }
}

fn check_test(test: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let Some(condition) = parse(test, source) else {
        return;
    };
    let mut checker = Checker {
        single_bracket: !is_double_bracket(test, source),
        ksh: context.dialect == Dialect::Ksh,
        source,
        findings,
    };
    condition.visit(true, &mut |part, _| match part {
        Condition::Nullary(word) => checker.nullary(*word),
        Condition::Unary(operator, word) => checker.unary(*operator, *word),
        Condition::Binary(operator, left, right) => checker.binary(*operator, *left, *right),
        Condition::And(..) | Condition::Or(..) | Condition::Not(..) | Condition::Group(_) => {}
    });
}

struct Checker<'run, 'source> {
    single_bracket: bool,
    ksh: bool,
    source: &'source [u8],
    findings: &'run mut Findings,
}

impl Checker<'_, '_> {
    fn report(&mut self, rule: ShellRule, word: Node<'_>) {
        self.findings.report(rule, word, text(word, self.source));
    }

    fn nullary(&mut self, word: Node<'_>) {
        let word_pieces = pieces(word, self.source);
        if is_constant(&word_pieces) {
            let rule = match literal_string(&word_pieces).unwrap_or_default().as_str() {
                FALSE_WORD => ShellRule::Sc2158,
                ZERO => ShellRule::Sc2159,
                TRUE_WORD => ShellRule::Sc2160,
                ONE => ShellRule::Sc2161,
                _ => ShellRule::Sc2078,
            };
            self.report(rule, word);
        } else if let Some(equals) = word_pieces.iter().find(|piece| has_equals(piece)) {
            self.findings
                .report_at(ShellRule::Sc2077, equals.position, text(word, self.source));
        } else {
            self.tautology(&word_pieces, word);
        }
        self.grep_output(Position::of(word), word);
        self.braces(word, &word_pieces);
        self.globs(word, &word_pieces);
        if !self.single_bracket {
            self.arrays(word, &word_pieces);
        }
    }

    fn tautology(&mut self, word_pieces: &[Piece], word: Node<'_>) {
        let literal_piece = flattened(word_pieces)
            .into_iter()
            .find(|piece| piece.literal_string().is_some_and(|literal| !literal.is_empty()));
        if let Some(literal_piece) = literal_piece {
            self.findings
                .report_at(ShellRule::Sc2157, literal_piece.position, text(word, self.source));
        }
    }

    fn unary(&mut self, operator: Node<'_>, word: Node<'_>) {
        let word_pieces = pieces(word, self.source);
        let operator_text = text(operator, self.source);
        if [NON_EMPTY, EMPTY].contains(&operator_text) {
            self.tautology(&word_pieces, word);
            self.grep_output(Position::of(operator), word);
        }
        if !has_glob(&word_pieces) {
            self.all(word, &word_pieces);
        } else if operator_text == VARIABLE_IS_SET {
            if self.single_bracket {
                self.report(ShellRule::Sc2208, word);
            }
        } else if !self.single_bracket || !self.ksh {
            self.report(ShellRule::Sc2144, word);
        } else if KSH_FIRST_GLOB_OPERATORS.contains(&operator_text) {
            self.report(ShellRule::Sc2245, word);
        }
    }

    fn binary(&mut self, operator: Node<'_>, left: Node<'_>, right: Node<'_>) {
        let operator_text = text(operator, self.source);
        let left_pieces = pieces(left, self.source);
        let right_pieces = pieces(right, self.source);
        self.constant_comparison(operator, &left_pieces, &right_pieces);
        if EQUALITY_OPERATORS.contains(&operator_text) {
            self.leading_x(left, &left_pieces, &right_pieces);
        }
        if operator_text == REGEX_MATCH {
            self.quoted_regex(right, &right_pieces);
        }
        if ARITHMETIC_OPERATORS.contains(&operator_text) {
            self.numeric_operand(left, &left_pieces);
            self.numeric_operand(right, &right_pieces);
        } else if STRING_OPERATORS.contains(&operator_text) {
            self.all(left, &left_pieces);
            self.arrays(right, &right_pieces);
            self.braces(right, &right_pieces);
        } else {
            self.all(left, &left_pieces);
            self.all(right, &right_pieces);
        }
    }

    fn numeric_operand(&mut self, word: Node<'_>, word_pieces: &[Piece]) {
        if self.single_bracket && !self.ksh && has_glob(word_pieces) {
            self.report(ShellRule::Sc2255, word);
        }
        self.arrays(word, word_pieces);
        self.braces(word, word_pieces);
    }

    // SC2050 y SC2193: `checkConstantIfs`.
    fn constant_comparison(&mut self, operator: Node<'_>, left: &[Piece], right: &[Piece]) {
        let operator_text = text(operator, self.source);
        let dynamic = (ARITHMETIC_OPERATORS.contains(&operator_text) && !self.single_bracket)
            || FILE_COMPARISONS.contains(&operator_text);
        if dynamic {
            return;
        }
        if is_constant(left) && is_constant(right) {
            self.report(ShellRule::Sc2050, operator);
        } else if EQUALITY_OPERATORS.contains(&operator_text) && !can_overlap(&fuzzy(left), &fuzzy(right)) {
            self.report(ShellRule::Sc2193, operator);
        }
    }

    fn all(&mut self, word: Node<'_>, word_pieces: &[Piece]) {
        self.arrays(word, word_pieces);
        self.braces(word, word_pieces);
        self.globs(word, word_pieces);
    }

    fn arrays(&mut self, word: Node<'_>, word_pieces: &[Piece]) {
        if has_array_expansion(word_pieces) {
            let rule = if self.single_bracket {
                ShellRule::Sc2198
            } else {
                ShellRule::Sc2199
            };
            self.report(rule, word);
        }
    }

    fn braces(&mut self, word: Node<'_>, word_pieces: &[Piece]) {
        if has_brace_expansion(word_pieces) {
            let rule = if self.single_bracket {
                ShellRule::Sc2200
            } else {
                ShellRule::Sc2201
            };
            self.report(rule, word);
        }
    }

    fn globs(&mut self, word: Node<'_>, word_pieces: &[Piece]) {
        if has_glob(word_pieces) {
            let rule = if self.single_bracket {
                ShellRule::Sc2202
            } else {
                ShellRule::Sc2203
            };
            self.report(rule, word);
        }
    }

    // SC2076: `checkQuotedCondRegex`.
    fn quoted_regex(&mut self, right: Node<'_>, right_pieces: &[Piece]) {
        let [only] = right_pieces else {
            return;
        };
        if !matches!(only.kind, PieceKind::DoubleQuoted | PieceKind::SingleQuoted) {
            return;
        }
        let constant_non_regex = only
            .literal_string()
            .is_some_and(|literal| !literal.contains(REGEX_METACHARACTERS));
        if !constant_non_regex {
            self.report(ShellRule::Sc2076, right);
        }
    }

    // SC2268: `checkComparisonWithLeadingX`.
    fn leading_x(&mut self, left: Node<'_>, left_pieces: &[Piece], right_pieces: &[Piece]) {
        if starts_with_x(left_pieces) && starts_with_x(right_pieces) {
            self.report(ShellRule::Sc2268, left);
        }
    }

    // SC2143: `checkShouldUseGrepQ`.
    fn grep_output(&mut self, position: Position, word: Node<'_>) {
        if let Some(name) = final_grep(word, self.source) {
            self.findings.report_at(ShellRule::Sc2143, position, &name);
        }
    }
}

fn has_equals(piece: &Piece) -> bool {
    piece
        .literal_string()
        .is_some_and(|literal| literal.contains(EQUALS))
}

fn starts_with_x(word_pieces: &[Piece]) -> bool {
    flattened(word_pieces).first().is_some_and(|first| {
        matches!(first.kind, PieceKind::Literal | PieceKind::SingleQuoted)
            && first
                .text
                .chars()
                .next()
                .is_some_and(|character| character.to_ascii_lowercase() == LEADING_X)
    })
}

fn check_test_command_leading_x(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(command) = Command::of(node) else {
        return;
    };
    let [name, left, operator, right] = command.words[..] else {
        return;
    };
    let operator_literal = literal_string(&pieces(operator, source)).unwrap_or_default();
    if literal_string(&pieces(name, source)).as_deref() == Some(TEST_COMMAND)
        && EQUALITY_OPERATORS.contains(&operator_literal.as_str())
        && starts_with_x(&pieces(left, source))
        && starts_with_x(&pieces(right, source))
    {
        findings.report(ShellRule::Sc2268, left, text(left, source));
    }
}

fn final_grep(word: Node<'_>, source: &[u8]) -> Option<String> {
    let substitution = sole_substitution(word)?;
    if is_backticked(substitution, source) {
        return None;
    }
    let [statement] = named_children(substitution)[..] else {
        return None;
    };
    let mut last = statement;
    if last.kind() == REDIRECTED {
        last = last.child_by_field_name(BODY_FIELD)?;
    }
    if last.kind() == PIPELINE {
        last = *named_children(last).last()?;
    }
    if last.kind() == REDIRECTED {
        last = last.child_by_field_name(BODY_FIELD)?;
    }
    let name = Command::of(last)?.name(source)?;
    let basename = name.rsplit(PATH_SEPARATOR).next()?;
    GREP_COMMANDS.contains(&basename).then(|| basename.to_owned())
}

fn sole_substitution(word: Node<'_>) -> Option<Node<'_>> {
    if word.kind() == COMMAND_SUBSTITUTION {
        return Some(word);
    }
    if word.kind() != STRING {
        return None;
    }
    let [only] = named_children(word)[..] else {
        return None;
    };
    (only.kind() == COMMAND_SUBSTITUTION).then_some(only)
}
