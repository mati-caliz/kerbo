// Port de go/build/constraint: lo justo para leer `//go:build` y `// +build`, escribir la expresión
// canónica y volver a generar las líneas `// +build` equivalentes.

const GO_BUILD_PREFIX: &str = "//go:build";
const LINE_COMMENT_PREFIX: &str = "//";
const PLUS_BUILD_PREFIX: &str = "+build";
const PLUS_BUILD_LINE: &str = "// +build";
const IGNORED_TAG: &str = "ignore";
const MAX_EXPRESSION_SIZE: usize = 1000;
const MAX_PLUS_BUILD_SIZE: usize = 100;
const AND: &str = "&&";
const OR: &str = "||";
const NOT: &str = "!";
const DOUBLE_NOT: &str = "!!";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
    Tag(String),
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
}

impl Constraint {
    pub fn render(&self) -> String {
        match self {
            Self::Tag(tag) => tag.clone(),
            Self::Not(inner) => {
                let text = inner.render();
                if matches!(**inner, Self::And(..) | Self::Or(..)) {
                    format!("!({text})")
                } else {
                    format!("!{text}")
                }
            }
            Self::And(left, right) => format!(
                "{} && {}",
                left.wrapped_in(Self::is_or),
                right.wrapped_in(Self::is_or)
            ),
            Self::Or(left, right) => format!(
                "{} || {}",
                left.wrapped_in(Self::is_and),
                right.wrapped_in(Self::is_and)
            ),
        }
    }

    fn wrapped_in(&self, needs_parens: fn(&Self) -> bool) -> String {
        if needs_parens(self) {
            format!("({})", self.render())
        } else {
            self.render()
        }
    }

    const fn is_and(&self) -> bool {
        matches!(self, Self::And(..))
    }

    const fn is_or(&self) -> bool {
        matches!(self, Self::Or(..))
    }
}

fn without_newline(line: &str) -> Option<&str> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    (!line.contains('\n')).then_some(line)
}

fn after_keyword(rest: &str) -> Option<&str> {
    let trimmed = rest.trim();
    (rest.len() != trimmed.len() || rest.is_empty()).then_some(trimmed)
}

pub fn split_go_build(line: &str) -> Option<&str> {
    let line = without_newline(line)?;
    if !line.starts_with(GO_BUILD_PREFIX) {
        return None;
    }
    after_keyword(line.trim().get(GO_BUILD_PREFIX.len()..)?)
}

pub fn split_plus_build(line: &str) -> Option<&str> {
    let line = without_newline(line)?;
    let rest = line.strip_prefix(LINE_COMMENT_PREFIX)?.trim();
    after_keyword(rest.strip_prefix(PLUS_BUILD_PREFIX)?)
}

pub fn parse(line: &str) -> Option<Constraint> {
    if let Some(text) = split_go_build(line) {
        let mut parser = Parser {
            text,
            index: 0,
            token: "",
            is_tag: false,
            size: 0,
        };
        let parsed = parser.or()?;
        return parser.token.is_empty().then_some(parsed);
    }
    parse_plus_build(split_plus_build(line)?)
}

struct Parser<'text> {
    text: &'text str,
    index: usize,
    token: &'text str,
    is_tag: bool,
    size: usize,
}

impl Parser<'_> {
    fn or(&mut self) -> Option<Constraint> {
        let mut left = self.and()?;
        while self.token == OR {
            left = Constraint::Or(Box::new(left), Box::new(self.and()?));
        }
        Some(left)
    }

    fn and(&mut self) -> Option<Constraint> {
        let mut left = self.not()?;
        while self.token == AND {
            left = Constraint::And(Box::new(left), Box::new(self.not()?));
        }
        Some(left)
    }

    fn not(&mut self) -> Option<Constraint> {
        self.size = self.size.saturating_add(1);
        if self.size > MAX_EXPRESSION_SIZE {
            return None;
        }
        self.lex()?;
        if self.token == NOT {
            self.lex()?;
            if self.token == NOT {
                return None;
            }
            return Some(Constraint::Not(Box::new(self.atom()?)));
        }
        self.atom()
    }

    fn atom(&mut self) -> Option<Constraint> {
        if self.token == "(" {
            let inner = self.or()?;
            if self.token != ")" {
                return None;
            }
            self.lex()?;
            return Some(inner);
        }
        if !self.is_tag {
            return None;
        }
        let tag = self.token.to_owned();
        self.lex()?;
        Some(Constraint::Tag(tag))
    }

    fn lex(&mut self) -> Option<()> {
        self.is_tag = false;
        let rest = self.text.get(self.index..)?;
        let trimmed = rest.trim_start_matches([' ', '\t']);
        self.index = self
            .index
            .saturating_add(rest.len().saturating_sub(trimmed.len()));
        let start = self.index;
        let length = match trimmed.bytes().next() {
            None => 0,
            Some(b'(' | b')' | b'!') => 1,
            Some(byte @ (b'&' | b'|')) => {
                if trimmed.as_bytes().get(1) != Some(&byte) {
                    return None;
                }
                2
            }
            Some(_) => {
                let tag = trimmed
                    .find(|character| !is_tag_character(character))
                    .unwrap_or(trimmed.len());
                if tag == 0 {
                    return None;
                }
                self.is_tag = true;
                tag
            }
        };
        self.index = start.saturating_add(length);
        self.token = self.text.get(start..self.index)?;
        Some(())
    }
}

fn is_tag_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '.'
}

fn plus_build_literal(literal: &str) -> Constraint {
    if literal.starts_with(DOUBLE_NOT) || literal == NOT {
        return Constraint::Tag(IGNORED_TAG.to_owned());
    }
    let (negated, name) = literal
        .strip_prefix(NOT)
        .map_or((false, literal), |rest| (true, rest));
    let valid = !name.is_empty() && name.chars().all(is_tag_character);
    let tag = Constraint::Tag(if valid { name } else { IGNORED_TAG }.to_owned());
    if negated {
        Constraint::Not(Box::new(tag))
    } else {
        tag
    }
}

fn parse_plus_build(text: &str) -> Option<Constraint> {
    let mut size = 0;
    let mut result: Option<Constraint> = None;
    for clause in text.split_whitespace() {
        let mut conjunction: Option<Constraint> = None;
        for literal in clause.split(',') {
            let current = plus_build_literal(literal);
            conjunction = Some(match conjunction {
                None => current,
                Some(previous) => {
                    size = bounded(size)?;
                    Constraint::And(Box::new(previous), Box::new(current))
                }
            });
        }
        let Some(conjunction) = conjunction else {
            continue;
        };
        result = Some(match result {
            None => conjunction,
            Some(previous) => {
                size = bounded(size)?;
                Constraint::Or(Box::new(previous), Box::new(conjunction))
            }
        });
    }
    Some(result.unwrap_or_else(|| Constraint::Tag(IGNORED_TAG.to_owned())))
}

fn bounded(size: usize) -> Option<usize> {
    let next = size.saturating_add(1);
    (next <= MAX_PLUS_BUILD_SIZE).then_some(next)
}

fn push_not(x: &Constraint, negate: bool) -> Constraint {
    match x {
        Constraint::Not(inner) => {
            if matches!(**inner, Constraint::Tag(_)) && !negate {
                return x.clone();
            }
            push_not(inner, !negate)
        }
        Constraint::Tag(_) => {
            if negate {
                Constraint::Not(Box::new(x.clone()))
            } else {
                x.clone()
            }
        }
        Constraint::And(left, right) => {
            let (left, right) = (
                Box::new(push_not(left, negate)),
                Box::new(push_not(right, negate)),
            );
            if negate {
                Constraint::Or(left, right)
            } else {
                Constraint::And(left, right)
            }
        }
        Constraint::Or(left, right) => {
            let (left, right) = (
                Box::new(push_not(left, negate)),
                Box::new(push_not(right, negate)),
            );
            if negate {
                Constraint::And(left, right)
            } else {
                Constraint::Or(left, right)
            }
        }
    }
}

fn split_and(x: &Constraint, list: &mut Vec<Constraint>) {
    if let Constraint::And(left, right) = x {
        split_and(left, list);
        split_and(right, list);
    } else {
        list.push(x.clone());
    }
}

fn split_or(x: &Constraint, list: &mut Vec<Constraint>) {
    if let Constraint::Or(left, right) = x {
        split_or(left, list);
        split_or(right, list);
    } else {
        list.push(x.clone());
    }
}

pub fn plus_build_lines(x: &Constraint) -> Option<Vec<String>> {
    let mut clauses = Vec::new();
    split_and(&push_not(x, false), &mut clauses);
    let mut split: Vec<Vec<Vec<Constraint>>> = Vec::new();
    for clause in &clauses {
        let mut alternatives = Vec::new();
        split_or(clause, &mut alternatives);
        let mut ors = Vec::new();
        for alternative in &alternatives {
            let mut literals = Vec::new();
            split_and(alternative, &mut literals);
            if literals
                .iter()
                .any(|literal| matches!(literal, Constraint::And(..) | Constraint::Or(..)))
            {
                return None;
            }
            ors.push(literals);
        }
        split.push(ors);
    }
    if split.iter().all(|ors| ors.len() <= 1) && split.iter().any(|ors| ors.len() == 1) {
        let literals = split
            .into_iter()
            .flat_map(|ors| ors.into_iter().flatten())
            .collect();
        split = vec![vec![literals]];
    }
    Some(split.iter().map(|ors| plus_build_line(ors)).collect())
}

fn plus_build_line(ors: &[Vec<Constraint>]) -> String {
    let mut line = PLUS_BUILD_LINE.to_owned();
    for and in ors {
        line.push(' ');
        let literals: Vec<String> = and.iter().map(Constraint::render).collect();
        line.push_str(&literals.join(","));
    }
    line
}
