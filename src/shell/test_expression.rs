use tree_sitter::Node;

use super::syntax::{
    ANSI_STRING, ARITHMETIC_EXPANSION, COMMAND_SUBSTITUTION, CONCATENATION, EXPANSION, NUMBER, RAW_STRING,
    SIMPLE_EXPANSION, STRING, TEST, TEST_OPERATOR, WORD, children, text,
};

const OPERAND_KINDS: &[&str] = &[
    WORD,
    STRING,
    RAW_STRING,
    ANSI_STRING,
    CONCATENATION,
    SIMPLE_EXPANSION,
    EXPANSION,
    COMMAND_SUBSTITUTION,
    ARITHMETIC_EXPANSION,
    NUMBER,
    TEST_OPERATOR,
    "extglob_pattern",
    "regex",
];
const AND_OPERATORS: &[&str] = &["&&", "-a"];
const OR_OPERATORS: &[&str] = &["||", "-o"];
const FLAGLESS_BINARY: &[&str] = &["==", "!=", "<=", ">=", "=~", ">", "<", "="];
const OPEN_GROUP: &[&str] = &["(", "\\("];
const CLOSE_GROUP: &[&str] = &[")", "\\)"];
const NEGATION: &str = "!";
const FLAG_PREFIX: char = '-';
const BRACKETS: &[&str] = &["[", "]", "[[", "]]"];

#[derive(Debug, Clone)]
pub enum Condition<'tree> {
    And(Node<'tree>, Box<Self>, Box<Self>),
    Or(Node<'tree>, Box<Self>, Box<Self>),
    Not(Box<Self>),
    Group(Box<Self>),
    Unary,
    Binary(Node<'tree>, Node<'tree>, Node<'tree>),
    Nullary,
}

struct Reader<'tree, 'source> {
    tokens: Vec<Node<'tree>>,
    position: usize,
    source: &'source [u8],
}

pub fn parse<'tree>(test: Node<'tree>, source: &[u8]) -> Option<Condition<'tree>> {
    if test.kind() != TEST {
        return None;
    }
    let mut tokens = Vec::new();
    collect(test, source, &mut tokens);
    let mut reader = Reader {
        tokens,
        position: 0,
        source,
    };
    let condition = reader.and_chain()?;
    (reader.position == reader.tokens.len()).then_some(condition)
}

fn collect<'tree>(node: Node<'tree>, source: &[u8], tokens: &mut Vec<Node<'tree>>) {
    for child in children(node) {
        if OPERAND_KINDS.contains(&child.kind()) {
            tokens.push(child);
        } else if child.child_count() == 0 {
            if !BRACKETS.contains(&text(child, source)) {
                tokens.push(child);
            }
        } else {
            collect(child, source, tokens);
        }
    }
}

impl<'tree> Reader<'tree, '_> {
    fn peek(&self) -> Option<Node<'tree>> {
        self.tokens.get(self.position).copied()
    }

    fn peek_text(&self) -> &str {
        self.peek().map_or("", |token| text(token, self.source))
    }

    fn advance(&mut self) -> Option<Node<'tree>> {
        let token = self.peek()?;
        self.position = self.position.saturating_add(1);
        Some(token)
    }

    fn and_chain(&mut self) -> Option<Condition<'tree>> {
        let mut left = self.or_chain()?;
        while AND_OPERATORS.contains(&self.peek_text()) {
            let operator = self.advance()?;
            let right = self.or_chain()?;
            left = Condition::And(operator, Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn or_chain(&mut self) -> Option<Condition<'tree>> {
        let mut left = self.term()?;
        while OR_OPERATORS.contains(&self.peek_text()) {
            let operator = self.advance()?;
            let right = self.term()?;
            left = Condition::Or(operator, Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn term(&mut self) -> Option<Condition<'tree>> {
        if self.peek_text() == NEGATION {
            self.advance()?;
            return Some(Condition::Not(Box::new(self.expression()?)));
        }
        self.expression()
    }

    fn expression(&mut self) -> Option<Condition<'tree>> {
        if OPEN_GROUP.contains(&self.peek_text()) {
            self.advance()?;
            let inner = self.and_chain()?;
            if !CLOSE_GROUP.contains(&self.peek_text()) {
                return None;
            }
            self.advance()?;
            return Some(Condition::Group(Box::new(inner)));
        }
        if is_flag_operator(self.peek_text()) {
            self.advance()?;
            self.advance()?;
            return Some(Condition::Unary);
        }
        let left = self.advance()?;
        if self.is_binary_operator() {
            let operator = self.advance()?;
            let right = self.advance()?;
            return Some(Condition::Binary(operator, left, right));
        }
        Some(Condition::Nullary)
    }

    fn is_binary_operator(&self) -> bool {
        let operator = self.peek_text();
        (is_flag_operator(operator)
            && !AND_OPERATORS.contains(&operator)
            && !OR_OPERATORS.contains(&operator))
            || FLAGLESS_BINARY.contains(&operator)
    }
}

fn is_flag_operator(text: &str) -> bool {
    text.strip_prefix(FLAG_PREFIX).is_some_and(|letters| {
        !letters.is_empty() && letters.chars().all(|letter| letter.is_ascii_alphabetic())
    })
}

impl Condition<'_> {
    pub fn visit(&self, only_test: bool, visitor: &mut impl FnMut(&Self, bool)) {
        visitor(self, only_test);
        match self {
            Self::And(_, left, right) | Self::Or(_, left, right) => {
                left.visit(false, visitor);
                right.visit(false, visitor);
            }
            Self::Not(inner) | Self::Group(inner) => inner.visit(only_test, visitor),
            Self::Unary | Self::Binary(..) | Self::Nullary => {}
        }
    }
}
