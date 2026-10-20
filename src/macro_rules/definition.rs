use super::tokens::{Delimiter, Leaf, LeafKind, Origin, TokenTree};

const METAVARIABLE_MARK: &str = "$";
const KIND_SEPARATOR: &str = ":";
const RULE_ARROW: &str = "=>";
const RULE_SEPARATOR: &str = ";";
const CRATE_VARIABLE: &str = "crate";
const MACRO_BANG: &str = "!";
const FUNCTION_KEYWORD: &str = "fn";
const ZERO_OR_MORE: &str = "*";
const ONE_OR_MORE: &str = "+";
const ZERO_OR_ONE: &str = "?";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentKind {
    Block,
    Expr,
    Ident,
    Item,
    Lifetime,
    Literal,
    Meta,
    Pat,
    PatParam,
    Path,
    Stmt,
    Tt,
    Ty,
    Vis,
}

impl FragmentKind {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "block" => Self::Block,
            "expr" | "expr_2021" => Self::Expr,
            "ident" => Self::Ident,
            "item" => Self::Item,
            "lifetime" => Self::Lifetime,
            "literal" => Self::Literal,
            "meta" => Self::Meta,
            "pat" => Self::Pat,
            "pat_param" => Self::PatParam,
            "path" => Self::Path,
            "stmt" => Self::Stmt,
            "tt" => Self::Tt,
            "ty" => Self::Ty,
            "vis" => Self::Vis,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

impl Repeat {
    fn from_mark(tree: &TokenTree) -> Option<Self> {
        [
            (ZERO_OR_MORE, Self::ZeroOrMore),
            (ONE_OR_MORE, Self::OneOrMore),
            (ZERO_OR_ONE, Self::ZeroOrOne),
        ]
        .into_iter()
        .find(|(mark, _)| tree.is_punct(mark))
        .map(|(_, repeat)| repeat)
    }
}

#[derive(Debug, Clone)]
pub struct Repetition<Item> {
    pub items: Vec<Item>,
    pub separator: Option<Leaf>,
    pub repeat: Repeat,
}

#[derive(Debug, Clone)]
pub enum Matcher {
    Token(Leaf),
    Group(Delimiter, Vec<Self>),
    Fragment(String, FragmentKind),
    Repetition(Repetition<Self>),
}

#[derive(Debug, Clone)]
pub enum Transcriber {
    Token(Leaf),
    Group(Delimiter, Origin, Origin, Vec<Self>),
    Variable(String),
    Crate(Origin),
    Repetition(Repetition<Self>),
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub matcher: Vec<Matcher>,
    pub transcriber: Vec<Transcriber>,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub rules: Vec<Rule>,
    pub origin: Origin,
    pub mentions: Vec<String>,
    pub visible_in: Option<(usize, usize)>,
    pub shared: bool,
    pub writes_functions: bool,
}

impl Definition {
    pub fn is_visible_at(&self, at: Origin) -> bool {
        self.origin.source == at.source
            && self
                .visible_in
                .is_none_or(|(start, end)| start <= at.offset && at.offset < end)
    }
}

pub fn mentioned_macros(rules: &[Rule]) -> Vec<String> {
    let mut mentions = Vec::new();
    for rule in rules {
        collect_mentions(&rule.transcriber, &mut mentions);
    }
    mentions
}

fn collect_mentions(items: &[Transcriber], mentions: &mut Vec<String>) {
    for pair in items.windows(2) {
        if let [Transcriber::Token(name), Transcriber::Token(bang)] = pair
            && name.kind == LeafKind::Ident
            && bang.text == MACRO_BANG
            && !mentions.contains(&name.text)
        {
            mentions.push(name.text.clone());
        }
    }
    for item in items {
        match item {
            Transcriber::Group(_, _, _, inner) => collect_mentions(inner, mentions),
            Transcriber::Repetition(repetition) => collect_mentions(&repetition.items, mentions),
            Transcriber::Token(_) | Transcriber::Variable(_) | Transcriber::Crate(_) => {}
        }
    }
}

pub fn writes_functions(rules: &[Rule]) -> bool {
    rules.iter().any(|rule| has_function_keyword(&rule.transcriber))
}

fn has_function_keyword(items: &[Transcriber]) -> bool {
    items.iter().any(|item| match item {
        Transcriber::Token(leaf) => leaf.kind == LeafKind::Ident && leaf.text == FUNCTION_KEYWORD,
        Transcriber::Group(_, _, _, inner) => has_function_keyword(inner),
        Transcriber::Repetition(repetition) => has_function_keyword(&repetition.items),
        Transcriber::Variable(_) | Transcriber::Crate(_) => false,
    })
}

pub fn parse_rules(trees: &[TokenTree]) -> Option<Vec<Rule>> {
    let mut rules = Vec::new();
    let mut rest = trees;
    while let [matcher, arrow, transcriber, after @ ..] = rest {
        if !arrow.is_punct(RULE_ARROW) {
            return None;
        }
        rules.push(Rule {
            matcher: matcher_items(&matcher.group()?.trees)?,
            transcriber: transcriber_items(&transcriber.group()?.trees)?,
        });
        rest = match after {
            [separator, remaining @ ..] if separator.is_punct(RULE_SEPARATOR) => remaining,
            _ => after,
        };
    }
    rest.is_empty().then_some(rules)
}

fn matcher_items(trees: &[TokenTree]) -> Option<Vec<Matcher>> {
    let mut items = Vec::new();
    let mut rest = trees;
    while let Some((first, after)) = rest.split_first() {
        let (item, remaining) = match (first, after) {
            (
                mark,
                [
                    TokenTree::Leaf(name),
                    separator,
                    TokenTree::Leaf(kind),
                    remaining @ ..,
                ],
            ) if mark.is_punct(METAVARIABLE_MARK)
                && name.kind == LeafKind::Ident
                && separator.is_punct(KIND_SEPARATOR) =>
            {
                (
                    Matcher::Fragment(name.text.clone(), FragmentKind::named(&kind.text)?),
                    remaining,
                )
            }
            (mark, [TokenTree::Group(group), suffix @ ..])
                if mark.is_punct(METAVARIABLE_MARK) && group.delimiter == Delimiter::Parenthesis =>
            {
                let (separator, repeat, remaining) = repetition_suffix(suffix)?;
                let repeated = matcher_items(&group.trees)?;
                (
                    Matcher::Repetition(Repetition {
                        items: repeated,
                        separator,
                        repeat,
                    }),
                    remaining,
                )
            }
            (TokenTree::Group(group), _) => (
                Matcher::Group(group.delimiter, matcher_items(&group.trees)?),
                after,
            ),
            (TokenTree::Leaf(leaf), _) => (Matcher::Token(leaf.clone()), after),
        };
        items.push(item);
        rest = remaining;
    }
    Some(items)
}

fn transcriber_items(trees: &[TokenTree]) -> Option<Vec<Transcriber>> {
    let mut items = Vec::new();
    let mut rest = trees;
    while let Some((first, after)) = rest.split_first() {
        let (item, remaining) = match (first, after) {
            (TokenTree::Leaf(mark), [TokenTree::Leaf(name), remaining @ ..])
                if mark.text == METAVARIABLE_MARK && name.kind == LeafKind::Ident =>
            {
                let item = if name.text == CRATE_VARIABLE {
                    Transcriber::Crate(name.origin)
                } else {
                    Transcriber::Variable(name.text.clone())
                };
                (item, remaining)
            }
            (mark, [TokenTree::Group(group), suffix @ ..])
                if mark.is_punct(METAVARIABLE_MARK) && group.delimiter == Delimiter::Parenthesis =>
            {
                let (separator, repeat, remaining) = repetition_suffix(suffix)?;
                let repeated = transcriber_items(&group.trees)?;
                (
                    Transcriber::Repetition(Repetition {
                        items: repeated,
                        separator,
                        repeat,
                    }),
                    remaining,
                )
            }
            (TokenTree::Group(group), _) => (
                Transcriber::Group(
                    group.delimiter,
                    group.open,
                    group.close,
                    transcriber_items(&group.trees)?,
                ),
                after,
            ),
            (TokenTree::Leaf(leaf), _) => (Transcriber::Token(leaf.clone()), after),
        };
        items.push(item);
        rest = remaining;
    }
    Some(items)
}

fn repetition_suffix(suffix: &[TokenTree]) -> Option<(Option<Leaf>, Repeat, &[TokenTree])> {
    let (first, rest) = suffix.split_first()?;
    if let Some(repeat) = Repeat::from_mark(first) {
        return Some((None, repeat, rest));
    }
    let (second, remaining) = rest.split_first()?;
    let repeat = Repeat::from_mark(second)?;
    Some((Some(first.leaf()?.clone()), repeat, remaining))
}
