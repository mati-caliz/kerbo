use super::fragments::{angle_arguments_length, type_length};
use super::tokens::{Delimiter, LeafKind, TokenTree};

const PREFIX_OPERATORS: &[&str] = &["-", "!", "*", "&", "&&"];
const BINARY_OPERATORS: &[&str] = &[
    "+", "-", "*", "/", "%", "^", "&", "|", "&&", "||", "<<", ">>", "==", "!=", "<", ">", "<=", ">=", "=",
    "+=", "-=", "*=", "/=", "%=", "^=", "&=", "|=", "<<=", ">>=", "..", "..=",
];
const RANGES: &[&str] = &["..", "..="];
const MUTABLE: &str = "mut";
const RAW: &str = "raw";
const QUESTION: &str = "?";
const DOT: &str = ".";
const AWAIT: &str = "await";
const CAST: &str = "as";
const PATH_SEPARATOR: &str = "::";
const ANGLE_OPEN: &str = "<";
const BANG: &str = "!";
const HASH: &str = "#";
const COLON: &str = ":";
const PIPE: &str = "|";
const EMPTY_PIPES: &str = "||";
const ARROW: &str = "->";
const ASSIGN: &str = "=";
const IF: &str = "if";
const ELSE: &str = "else";
const WHILE: &str = "while";
const MATCH: &str = "match";
const FOR: &str = "for";
const IN: &str = "in";
const LET: &str = "let";
const MOVE: &str = "move";
const ASYNC: &str = "async";
const RETURN: &str = "return";
const BREAK: &str = "break";
const CONTINUE: &str = "continue";
const YIELD: &str = "yield";
const BLOCK_KEYWORDS: &[&str] = &["loop", "unsafe", "const", "try"];
const NOT_OPERANDS: &[&str] = &[
    "as", "else", "enum", "extern", "fn", "impl", "in", "mod", "pub", "struct", "trait", "type", "use",
    "where", "mut", "ref", "dyn", "static",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Structs {
    Allowed,
    Forbidden,
}

// La extensión de un fragmento `expr` es la de la expresión más larga que arranca ahí, como la
// arma el parser de rustc: termina donde dos operandos quedan pegados sin un operador en el medio
// (`$($n:expr)*` sobre `0 1 i8::MIN` toma tres expresiones).
pub fn expression_length(trees: &[TokenTree]) -> Option<usize> {
    expression(trees, 0, Structs::Allowed).filter(|&end| end > 0)
}

fn expression(trees: &[TokenTree], start: usize, structs: Structs) -> Option<usize> {
    let mut position = operand(trees, start, structs)?;
    while let Some(operator) = trees
        .get(position)
        .filter(|candidate| is_any(candidate, BINARY_OPERATORS))
    {
        let next = position.saturating_add(1);
        if is_any(operator, RANGES)
            && !trees
                .get(next)
                .is_some_and(|candidate| begins_operand(candidate, structs))
        {
            return Some(next);
        }
        position = operand(trees, next, structs)?;
    }
    Some(position)
}

fn operand(trees: &[TokenTree], start: usize, structs: Structs) -> Option<usize> {
    let mut position = start;
    while let Some(tree) = trees.get(position) {
        let is_prefix = is_any(tree, PREFIX_OPERATORS)
            || (tree.is_word(MUTABLE) || tree.is_word(RAW)) && position > start;
        if !is_prefix {
            break;
        }
        position = position.saturating_add(1);
    }
    if trees
        .get(position)
        .is_some_and(|candidate| is_any(candidate, RANGES))
    {
        let next = position.saturating_add(1);
        return Some(
            if trees
                .get(next)
                .is_some_and(|candidate| begins_operand(candidate, structs))
            {
                operand(trees, next, structs)?
            } else {
                next
            },
        );
    }
    let primary_end = primary(trees, position, structs)?;
    Some(postfix(trees, primary_end))
}

fn postfix(trees: &[TokenTree], mut position: usize) -> usize {
    loop {
        let Some(tree) = trees.get(position) else {
            return position;
        };
        let next = position.saturating_add(1);
        position = if tree.is_punct(QUESTION) {
            next
        } else if tree.is_punct(DOT) {
            member(trees, next)
        } else if tree.delimited_by(Delimiter::Parenthesis) || tree.delimited_by(Delimiter::Bracket) {
            next
        } else if tree.is_word(CAST) {
            next.saturating_add(type_length(trees.get(next..).unwrap_or_default()).unwrap_or(0))
        } else {
            return position;
        };
    }
}

fn member(trees: &[TokenTree], start: usize) -> usize {
    let mut position = start.saturating_add(1);
    if trees.get(start).is_some_and(|candidate| candidate.is_word(AWAIT)) {
        return position;
    }
    if trees
        .get(position)
        .is_some_and(|candidate| candidate.is_punct(PATH_SEPARATOR))
        && trees
            .get(position.saturating_add(1))
            .is_some_and(|candidate| candidate.is_punct(ANGLE_OPEN))
    {
        let arguments = trees.get(position.saturating_add(1)..).unwrap_or_default();
        position = position
            .saturating_add(1)
            .saturating_add(angle_arguments_length(arguments));
    }
    position
}

fn primary(trees: &[TokenTree], position: usize, structs: Structs) -> Option<usize> {
    let tree = trees.get(position)?;
    let next = position.saturating_add(1);
    match tree {
        TokenTree::Group(_) => Some(next),
        TokenTree::Leaf(leaf) => match leaf.kind {
            LeafKind::Literal => Some(next),
            LeafKind::Lifetime if trees.get(next).is_some_and(|candidate| candidate.is_punct(COLON)) => {
                primary(trees, next.saturating_add(1), structs)
            }
            LeafKind::Lifetime => None,
            LeafKind::Punct => punctuated_primary(trees, position, structs),
            LeafKind::Ident => keyword_primary(trees, position, structs).or_else(|| {
                (!NOT_OPERANDS.contains(&leaf.text.as_str())).then(|| path_primary(trees, position, structs))
            }),
        },
    }
}

fn punctuated_primary(trees: &[TokenTree], position: usize, structs: Structs) -> Option<usize> {
    let tree = trees.get(position)?;
    let next = position.saturating_add(1);
    if tree.is_punct(PIPE) || tree.is_punct(EMPTY_PIPES) {
        return closure(trees, position, structs);
    }
    if tree.is_punct(PATH_SEPARATOR) {
        return Some(path_primary(trees, position, structs));
    }
    if tree.is_punct(ANGLE_OPEN) {
        let qualified = angle_arguments_length(trees.get(position..).unwrap_or_default());
        return Some(path_primary(trees, position.saturating_add(qualified), structs));
    }
    if tree.is_punct(HASH)
        && trees
            .get(next)
            .is_some_and(|candidate| candidate.delimited_by(Delimiter::Bracket))
    {
        return operand(trees, next.saturating_add(1), structs);
    }
    None
}

fn keyword_primary(trees: &[TokenTree], position: usize, structs: Structs) -> Option<usize> {
    let tree = trees.get(position)?;
    let next = position.saturating_add(1);
    if tree.is_word(IF) {
        return conditional(trees, position);
    }
    if tree.is_word(WHILE) {
        return braced_after(trees, condition(trees, next)?);
    }
    if tree.is_word(MATCH) {
        return braced_after(trees, expression(trees, next, Structs::Forbidden)?);
    }
    if tree.is_word(FOR) {
        let pattern_end = trees
            .iter()
            .enumerate()
            .skip(next)
            .find(|(_, candidate)| candidate.is_word(IN))
            .map(|(index, _)| index)?;
        return braced_after(
            trees,
            expression(trees, pattern_end.saturating_add(1), Structs::Forbidden)?,
        );
    }
    if tree.is_word(LET) {
        return condition(trees, position);
    }
    if tree.is_word(MOVE) {
        return closure(trees, next, structs);
    }
    if tree.is_word(ASYNC) {
        let after_move = if trees.get(next).is_some_and(|candidate| candidate.is_word(MOVE)) {
            next.saturating_add(1)
        } else {
            next
        };
        return braced_after(trees, after_move).or_else(|| closure(trees, after_move, structs));
    }
    if BLOCK_KEYWORDS.iter().any(|keyword| tree.is_word(keyword)) {
        return braced_after(trees, next);
    }
    if [RETURN, BREAK, YIELD].iter().any(|keyword| tree.is_word(keyword)) {
        let after_label = skip_label(trees, next);
        return Some(
            if trees
                .get(after_label)
                .is_some_and(|candidate| begins_operand(candidate, structs))
            {
                expression(trees, after_label, structs)?
            } else {
                after_label
            },
        );
    }
    if tree.is_word(CONTINUE) {
        return Some(skip_label(trees, next));
    }
    None
}

fn conditional(trees: &[TokenTree], position: usize) -> Option<usize> {
    let after_block = braced_after(trees, condition(trees, position.saturating_add(1))?)?;
    if !trees
        .get(after_block)
        .is_some_and(|candidate| candidate.is_word(ELSE))
    {
        return Some(after_block);
    }
    let alternative = after_block.saturating_add(1);
    if trees
        .get(alternative)
        .is_some_and(|candidate| candidate.is_word(IF))
    {
        return conditional(trees, alternative);
    }
    braced_after(trees, alternative)
}

fn condition(trees: &[TokenTree], start: usize) -> Option<usize> {
    if !trees.get(start).is_some_and(|candidate| candidate.is_word(LET)) {
        return expression(trees, start, Structs::Forbidden);
    }
    let assign = trees
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, candidate)| candidate.is_punct(ASSIGN))
        .map(|(index, _)| index)?;
    expression(trees, assign.saturating_add(1), Structs::Forbidden)
}

fn braced_after(trees: &[TokenTree], position: usize) -> Option<usize> {
    trees
        .get(position)
        .filter(|candidate| candidate.delimited_by(Delimiter::Brace))
        .map(|_| position.saturating_add(1))
}

fn skip_label(trees: &[TokenTree], position: usize) -> usize {
    let is_label = trees
        .get(position)
        .and_then(TokenTree::leaf)
        .is_some_and(|leaf| leaf.kind == LeafKind::Lifetime);
    if is_label {
        position.saturating_add(1)
    } else {
        position
    }
}

fn closure(trees: &[TokenTree], position: usize, structs: Structs) -> Option<usize> {
    let opening = trees.get(position)?;
    let after_parameters = if opening.is_punct(EMPTY_PIPES) {
        position.saturating_add(1)
    } else if opening.is_punct(PIPE) {
        trees
            .iter()
            .enumerate()
            .skip(position.saturating_add(1))
            .find(|(_, candidate)| candidate.is_punct(PIPE))
            .map(|(index, _)| index.saturating_add(1))?
    } else {
        return None;
    };
    if trees
        .get(after_parameters)
        .is_some_and(|candidate| candidate.is_punct(ARROW))
    {
        let return_type = after_parameters.saturating_add(1);
        let type_end = return_type.saturating_add(type_length(trees.get(return_type..).unwrap_or_default())?);
        return braced_after(trees, type_end);
    }
    expression(trees, after_parameters, structs)
}

fn path_primary(trees: &[TokenTree], start: usize, structs: Structs) -> usize {
    let mut position = start;
    let mut expects_segment = !trees
        .get(start)
        .is_some_and(|candidate| candidate.is_punct(PATH_SEPARATOR));
    while let Some(tree) = trees.get(position) {
        let is_segment = tree.leaf().is_some_and(|leaf| leaf.kind == LeafKind::Ident);
        if expects_segment && is_segment {
            expects_segment = false;
            position = position.saturating_add(1);
        } else if !expects_segment && tree.is_punct(PATH_SEPARATOR) {
            let after = position.saturating_add(1);
            if trees
                .get(after)
                .is_some_and(|candidate| candidate.is_punct(ANGLE_OPEN))
            {
                position =
                    after.saturating_add(angle_arguments_length(trees.get(after..).unwrap_or_default()));
            } else {
                expects_segment = true;
                position = after;
            }
        } else if position == start && tree.is_punct(PATH_SEPARATOR) {
            position = position.saturating_add(1);
            expects_segment = true;
        } else {
            break;
        }
    }
    let after = position.saturating_add(1);
    if trees
        .get(position)
        .is_some_and(|candidate| candidate.is_punct(BANG))
        && trees
            .get(after)
            .is_some_and(|candidate| candidate.group().is_some())
    {
        return after.saturating_add(1);
    }
    if structs == Structs::Allowed
        && trees
            .get(position)
            .is_some_and(|candidate| candidate.delimited_by(Delimiter::Brace))
    {
        return after;
    }
    position
}

fn begins_operand(tree: &TokenTree, structs: Structs) -> bool {
    match tree {
        TokenTree::Group(group) => structs == Structs::Allowed || group.delimiter != Delimiter::Brace,
        TokenTree::Leaf(leaf) => match leaf.kind {
            LeafKind::Literal | LeafKind::Lifetime => true,
            LeafKind::Ident => !NOT_OPERANDS.contains(&leaf.text.as_str()),
            LeafKind::Punct => {
                is_any(tree, PREFIX_OPERATORS)
                    || [PIPE, EMPTY_PIPES, PATH_SEPARATOR, ANGLE_OPEN, HASH]
                        .iter()
                        .any(|mark| tree.is_punct(mark))
            }
        },
    }
}

fn is_any(tree: &TokenTree, marks: &[&str]) -> bool {
    marks.iter().any(|mark| tree.is_punct(mark))
}
