const NOT: &str = "!";
const AND: &str = "&&";
const OR: &str = "||";
const OPEN: &str = "(";
const CLOSE: &str = ")";
const SINGLE_CHARACTER_TOKENS: &[char] = &['(', ')', '!'];
const DOUBLED_OPERATORS: &[char] = &['&', '|'];
const BLANKS: &[char] = &[' ', '\t'];
const TAG_PUNCTUATION: &[char] = &['_', '.'];
const PLUS_BUILD_AND: char = ',';
const DOUBLE_NOT: &str = "!!";
const IGNORE_TAG: &str = "ignore";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Tag(String),
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
}

impl Expression {
    pub fn evaluate(&self, matches: &impl Fn(&str) -> bool) -> bool {
        match self {
            Self::Tag(tag) => matches(tag),
            Self::Not(inner) => !inner.evaluate(matches),
            Self::And(left, right) => left.evaluate(matches) && right.evaluate(matches),
            Self::Or(left, right) => left.evaluate(matches) || right.evaluate(matches),
        }
    }

    fn tag(tag: &str) -> Self {
        Self::Tag(tag.to_owned())
    }
}

// `parseExpr` de `go/build/constraint`: `||`, `&&`, `!` sin doble negación y paréntesis.
pub fn go_build_expression(text: &str) -> Option<Expression> {
    let tokens = tokens(text)?;
    let mut parser = Parser { tokens, next: 0 };
    let expression = parser.or()?;
    (parser.next == parser.tokens.len()).then_some(expression)
}

// `parsePlusBuildExpr`: los espacios separan alternativas, las comas exigen todo y lo que no es un
// tag válido cuenta como `ignore`.
pub fn plus_build_expression(text: &str) -> Expression {
    text.split_whitespace()
        .map(|clause| {
            clause
                .split(PLUS_BUILD_AND)
                .map(plus_build_literal)
                .reduce(|left, right| Expression::And(Box::new(left), Box::new(right)))
                .unwrap_or_else(|| Expression::tag(IGNORE_TAG))
        })
        .reduce(|left, right| Expression::Or(Box::new(left), Box::new(right)))
        .unwrap_or_else(|| Expression::tag(IGNORE_TAG))
}

fn plus_build_literal(literal: &str) -> Expression {
    if literal.starts_with(DOUBLE_NOT) || literal == NOT {
        return Expression::tag(IGNORE_TAG);
    }
    let (negated, tag) = literal
        .strip_prefix(NOT)
        .map_or((false, literal), |tag| (true, tag));
    let expression = if is_valid_tag(tag) {
        Expression::tag(tag)
    } else {
        Expression::tag(IGNORE_TAG)
    };
    if negated {
        Expression::Not(Box::new(expression))
    } else {
        expression
    }
}

fn is_valid_tag(word: &str) -> bool {
    !word.is_empty() && word.chars().all(is_tag_character)
}

fn is_tag_character(character: char) -> bool {
    character.is_alphanumeric() || TAG_PUNCTUATION.contains(&character)
}

fn tokens(text: &str) -> Option<Vec<&str>> {
    let mut found = Vec::new();
    let mut rest = text.trim_start_matches(BLANKS);
    while let Some(first) = rest.chars().next() {
        let length = if SINGLE_CHARACTER_TOKENS.contains(&first) {
            first.len_utf8()
        } else if DOUBLED_OPERATORS.contains(&first) {
            let doubled = rest.chars().nth(1) == Some(first);
            doubled.then_some(first.len_utf8().saturating_mul(2))?
        } else {
            let tag_length = rest
                .find(|character| !is_tag_character(character))
                .unwrap_or(rest.len());
            (tag_length > 0).then_some(tag_length)?
        };
        let (token, after) = rest.split_at_checked(length)?;
        found.push(token);
        rest = after.trim_start_matches(BLANKS);
    }
    Some(found)
}

struct Parser<'text> {
    tokens: Vec<&'text str>,
    next: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.next).copied()
    }

    fn advance(&mut self) -> Option<&str> {
        let token = self.tokens.get(self.next).copied();
        self.next = self.next.saturating_add(1);
        token
    }

    fn or(&mut self) -> Option<Expression> {
        let mut expression = self.and()?;
        while self.peek() == Some(OR) {
            self.advance();
            expression = Expression::Or(Box::new(expression), Box::new(self.and()?));
        }
        Some(expression)
    }

    fn and(&mut self) -> Option<Expression> {
        let mut expression = self.not()?;
        while self.peek() == Some(AND) {
            self.advance();
            expression = Expression::And(Box::new(expression), Box::new(self.not()?));
        }
        Some(expression)
    }

    fn not(&mut self) -> Option<Expression> {
        if self.peek() != Some(NOT) {
            return self.atom();
        }
        self.advance();
        if self.peek() == Some(NOT) {
            return None;
        }
        Some(Expression::Not(Box::new(self.atom()?)))
    }

    fn atom(&mut self) -> Option<Expression> {
        let token = self.advance()?.to_owned();
        if token == OPEN {
            let inner = self.or()?;
            return (self.advance() == Some(CLOSE)).then_some(inner);
        }
        token
            .chars()
            .all(is_tag_character)
            .then(|| Expression::tag(&token))
    }
}
