use super::definition::FragmentKind;
use super::expressions::expression_length;
use super::tokens::{Delimiter, LeafKind, TokenTree};

const TYPE_STARTS: &[&str] = &["&", "&&", "*", "::", "<", "!"];
const NON_TYPE_WORDS: &[&str] = &[
    "as", "else", "enum", "if", "in", "let", "match", "mod", "pub", "return", "struct", "trait", "use",
    "where", "while", "loop", "mut", "ref", "static", "type",
];
const PATTERN_STOPS: &[&str] = &["=>", ",", "="];
const PATTERN_WORD_STOPS: &[&str] = &["if", "in"];
const ALTERNATIVE: &str = "|";
const TYPE_STOPS: &[&str] = &["=>", ",", "=", ":", ";", "|"];
const TYPE_WORD_STOPS: &[&str] = &["as", "where"];
const ANGLE_OPENS: &[(&str, usize)] = &[("<", 1), ("<<", 2)];
const ANGLE_CLOSES: &[(&str, usize)] = &[(">", 1), (">>", 2), (">=", 1), (">>=", 2)];
const PATH_SEPARATOR: &str = "::";
const UNDERSCORE: &str = "_";
const MINUS: &str = "-";
const BOOLEANS: &[&str] = &["true", "false"];
const PUBLIC: &str = "pub";
const RESTRICTIONS: &[&str] = &["crate", "self", "super", "in"];
const ASSIGN: &str = "=";
const LET: &str = "let";
const ELSE: &str = "else";
const ITEM_WORDS: &[&str] = &[
    "fn",
    "struct",
    "enum",
    "impl",
    "trait",
    "mod",
    "union",
    "const",
    "static",
    "type",
    "use",
    "extern",
    "unsafe",
    "async",
    "macro_rules",
    "pub",
];
const SEMICOLON_ITEMS: &[&str] = &["static", "type", "use"];
const CONSTANT: &str = "const";
const SEMICOLON: &str = ";";
const ATTRIBUTE: &str = "#";
const FUNCTION_QUALIFIERS: &[&str] = &["fn", "unsafe", "async", "extern"];

pub fn fragment_length(kind: FragmentKind, trees: &[TokenTree]) -> Option<usize> {
    let first = trees.first()?;
    match kind {
        FragmentKind::Tt => Some(1),
        FragmentKind::Ident => first
            .leaf()
            .filter(|leaf| leaf.kind == LeafKind::Ident && leaf.text != UNDERSCORE)
            .map(|_| 1),
        FragmentKind::Lifetime => first
            .leaf()
            .filter(|leaf| leaf.kind == LeafKind::Lifetime)
            .map(|_| 1),
        FragmentKind::Literal => literal_length(trees),
        FragmentKind::Block => first.delimited_by(Delimiter::Brace).then_some(1),
        FragmentKind::Vis => Some(visibility_length(trees)),
        FragmentKind::Meta => meta_length(trees),
        FragmentKind::Expr => expression_length(trees),
        FragmentKind::Stmt => statement_length(trees),
        FragmentKind::Pat => pattern_length(trees, true),
        FragmentKind::PatParam => pattern_length(trees, false),
        FragmentKind::Ty | FragmentKind::Path => type_length(trees),
        FragmentKind::Item => item_length(trees),
    }
}

fn literal_length(trees: &[TokenTree]) -> Option<usize> {
    let is_literal = |tree: &TokenTree| {
        tree.leaf().is_some_and(|leaf| {
            leaf.kind == LeafKind::Literal
                || (leaf.kind == LeafKind::Ident && BOOLEANS.contains(&leaf.text.as_str()))
        })
    };
    match trees {
        [minus, literal, ..] if minus.is_punct(MINUS) && is_literal(literal) => Some(2),
        [literal, ..] if is_literal(literal) => Some(1),
        _ => None,
    }
}

fn visibility_length(trees: &[TokenTree]) -> usize {
    match trees {
        [public, TokenTree::Group(restriction), ..]
            if public.is_word(PUBLIC)
                && restriction.delimiter == Delimiter::Parenthesis
                && restriction
                    .trees
                    .first()
                    .is_some_and(|word| RESTRICTIONS.iter().any(|restricted| word.is_word(restricted))) =>
        {
            2
        }
        [public, ..] if public.is_word(PUBLIC) => 1,
        _ => 0,
    }
}

fn meta_length(trees: &[TokenTree]) -> Option<usize> {
    let path = path_length(trees);
    if path == 0 {
        return None;
    }
    Some(match trees.get(path) {
        Some(TokenTree::Group(_)) => path.saturating_add(1),
        Some(assign) if assign.is_punct(ASSIGN) => trees.len(),
        _ => path,
    })
}

fn path_length(trees: &[TokenTree]) -> usize {
    let mut length = 0_usize;
    let mut expects_segment = true;
    for tree in trees {
        let is_segment = tree.leaf().is_some_and(|leaf| leaf.kind == LeafKind::Ident);
        if expects_segment && is_segment || tree.is_punct(PATH_SEPARATOR) {
            expects_segment = tree.is_punct(PATH_SEPARATOR);
            length = length.saturating_add(1);
        } else {
            break;
        }
    }
    length
}

pub fn angle_arguments_length(trees: &[TokenTree]) -> usize {
    let mut depth = 0_usize;
    for (index, tree) in trees.iter().enumerate() {
        if let Some(&(_, opened)) = ANGLE_OPENS.iter().find(|(mark, _)| tree.is_punct(mark)) {
            depth = depth.saturating_add(opened);
        } else if let Some(&(_, closed)) = ANGLE_CLOSES.iter().find(|(mark, _)| tree.is_punct(mark)) {
            depth = depth.saturating_sub(closed);
            if depth == 0 {
                return index.saturating_add(1);
            }
        }
    }
    trees.len()
}

fn statement_length(trees: &[TokenTree]) -> Option<usize> {
    let first = trees.first()?;
    if first.is_word(LET) {
        return let_statement_length(trees);
    }
    if starts_item(trees) {
        return item_length(trees);
    }
    expression_length(trees)
}

fn let_statement_length(trees: &[TokenTree]) -> Option<usize> {
    let Some(assign) = trees.iter().position(|tree| tree.is_punct(ASSIGN)) else {
        return trees
            .iter()
            .position(|tree| tree.is_punct(SEMICOLON))
            .or(Some(trees.len()));
    };
    let start = assign.saturating_add(1);
    let end = start.saturating_add(expression_length(trees.get(start..)?)?);
    let alternative = end.saturating_add(1);
    Some(
        if trees.get(end).is_some_and(|tree| tree.is_word(ELSE))
            && trees
                .get(alternative)
                .is_some_and(|tree| tree.delimited_by(Delimiter::Brace))
        {
            alternative.saturating_add(1)
        } else {
            end
        },
    )
}

fn pattern_length(trees: &[TokenTree], alternatives: bool) -> Option<usize> {
    let length = trees
        .iter()
        .position(|tree| {
            PATTERN_STOPS.iter().any(|stop| tree.is_punct(stop))
                || PATTERN_WORD_STOPS.iter().any(|stop| tree.is_word(stop))
                || (!alternatives && tree.is_punct(ALTERNATIVE))
        })
        .unwrap_or(trees.len());
    (length > 0).then_some(length)
}

fn can_begin_type(tree: &TokenTree) -> bool {
    match tree {
        TokenTree::Group(group) => group.delimiter != Delimiter::Brace,
        TokenTree::Leaf(leaf) => match leaf.kind {
            LeafKind::Ident => !NON_TYPE_WORDS.contains(&leaf.text.as_str()),
            LeafKind::Punct => TYPE_STARTS.contains(&leaf.text.as_str()),
            LeafKind::Literal | LeafKind::Lifetime => false,
        },
    }
}

pub fn type_length(trees: &[TokenTree]) -> Option<usize> {
    if !trees.first().is_some_and(can_begin_type) {
        return None;
    }
    let mut depth = 0_usize;
    for (index, tree) in trees.iter().enumerate() {
        if let Some(&(_, opened)) = ANGLE_OPENS.iter().find(|(mark, _)| tree.is_punct(mark)) {
            depth = depth.saturating_add(opened);
            continue;
        }
        if let Some(&(_, closed)) = ANGLE_CLOSES.iter().find(|(mark, _)| tree.is_punct(mark)) {
            if depth == 0 {
                return Some(index);
            }
            depth = depth.saturating_sub(closed);
            continue;
        }
        if depth == 0 && index > 0 && ends_type(tree) {
            return Some(index);
        }
    }
    Some(trees.len())
}

fn ends_type(tree: &TokenTree) -> bool {
    tree.delimited_by(Delimiter::Brace)
        || tree.delimited_by(Delimiter::Bracket)
        || TYPE_STOPS.iter().any(|stop| tree.is_punct(stop))
        || TYPE_WORD_STOPS.iter().any(|stop| tree.is_word(stop))
}

fn starts_item(trees: &[TokenTree]) -> bool {
    let start = attributes_length(trees);
    trees.get(start).is_some_and(|word| {
        ITEM_WORDS.iter().any(|item| word.is_word(item))
            && !(word.is_word(CONSTANT)
                && trees
                    .get(start.saturating_add(1))
                    .is_some_and(|next| next.delimited_by(Delimiter::Brace)))
    })
}

fn attributes_length(trees: &[TokenTree]) -> usize {
    let mut length = 0;
    while let [mark, TokenTree::Group(group), ..] = trees.get(length..).unwrap_or_default() {
        if !mark.is_punct(ATTRIBUTE) || group.delimiter != Delimiter::Bracket {
            break;
        }
        length = length.saturating_add(2);
    }
    length
}

fn item_length(trees: &[TokenTree]) -> Option<usize> {
    let start = attributes_length(trees);
    let body = trees.get(start..)?;
    let after_visibility = visibility_length(body);
    let keyword = body.get(after_visibility)?;
    let ends_with_semicolon = SEMICOLON_ITEMS.iter().any(|word| keyword.is_word(word))
        || (keyword.is_word(CONSTANT)
            && !body
                .get(after_visibility.saturating_add(1))
                .is_some_and(is_function_qualifier));
    let end = body
        .iter()
        .enumerate()
        .skip(after_visibility)
        .find_map(|(index, tree)| {
            let closes =
                tree.is_punct(SEMICOLON) || (!ends_with_semicolon && tree.delimited_by(Delimiter::Brace));
            closes.then_some(index.saturating_add(1))
        });
    let length = end.unwrap_or(body.len());
    (length > after_visibility).then(|| start.saturating_add(length))
}

fn is_function_qualifier(tree: &TokenTree) -> bool {
    FUNCTION_QUALIFIERS
        .iter()
        .any(|qualifier| tree.is_word(qualifier))
}
