use super::definition::{FragmentKind, Repetition, Transcriber};
use super::matcher::{Binding, Bindings};
use super::tokens::{Delimiter, Group, Leaf, LeafKind, Origin, TokenTree};

const CRATE_KEYWORD: &str = "crate";
const SPACE: char = ' ';

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub origin: Origin,
}

#[derive(Debug, Default)]
pub struct Rendered {
    pub text: String,
    pub spans: Vec<Span>,
}

pub fn transcribe(items: &[Transcriber], bindings: &Bindings) -> Option<Vec<TokenTree>> {
    let mut output = Vec::new();
    transcribe_into(items, bindings, &mut Vec::new(), &mut output)?;
    Some(output)
}

fn transcribe_into(
    items: &[Transcriber],
    bindings: &Bindings,
    path: &mut Vec<usize>,
    output: &mut Vec<TokenTree>,
) -> Option<()> {
    for item in items {
        match item {
            Transcriber::Token(leaf) => output.push(TokenTree::Leaf(leaf.clone())),
            Transcriber::Crate(origin) => output.push(TokenTree::Leaf(Leaf {
                kind: LeafKind::Ident,
                text: CRATE_KEYWORD.to_owned(),
                origin: *origin,
            })),
            Transcriber::Group(delimiter, open, close, inner) => {
                let mut trees = Vec::new();
                transcribe_into(inner, bindings, path, &mut trees)?;
                output.push(TokenTree::Group(Group {
                    delimiter: *delimiter,
                    open: *open,
                    close: *close,
                    trees,
                }));
            }
            Transcriber::Variable(name) => match lookup(bindings, name, path)? {
                Binding::Fragment(trees, kind) => push_fragment(trees, *kind, output),
                Binding::Repeated(_) => return None,
            },
            Transcriber::Repetition(repetition) => repeat(repetition, bindings, path, output)?,
        }
    }
    Some(())
}

fn repeat(
    repetition: &Repetition<Transcriber>,
    bindings: &Bindings,
    path: &mut Vec<usize>,
    output: &mut Vec<TokenTree>,
) -> Option<()> {
    let count =
        variable_names(&repetition.items)
            .iter()
            .find_map(|name| match lookup(bindings, name, path) {
                Some(Binding::Repeated(values)) => Some(values.len()),
                _ => None,
            })?;
    for index in 0..count {
        if index > 0
            && let Some(separator) = &repetition.separator
        {
            output.push(TokenTree::Leaf(separator.clone()));
        }
        path.push(index);
        let transcribed = transcribe_into(&repetition.items, bindings, path, output);
        path.pop();
        transcribed?;
    }
    Some(())
}

fn lookup<'bindings>(
    bindings: &'bindings Bindings,
    name: &str,
    path: &[usize],
) -> Option<&'bindings Binding> {
    let mut binding = bindings.get(name)?;
    for &index in path {
        match binding {
            Binding::Repeated(values) => binding = values.get(index)?,
            Binding::Fragment(..) => break,
        }
    }
    Some(binding)
}

fn variable_names(items: &[Transcriber]) -> Vec<&str> {
    items
        .iter()
        .flat_map(|item| match item {
            Transcriber::Variable(name) => vec![name.as_str()],
            Transcriber::Group(_, _, _, inner) => variable_names(inner),
            Transcriber::Repetition(repetition) => variable_names(&repetition.items),
            Transcriber::Token(_) | Transcriber::Crate(_) => Vec::new(),
        })
        .collect()
}

// Un fragmento ya parseado es opaco para rustc: `$e * 2` con `a + b` multiplica la suma. Al volver
// a texto, la expresión va entre paréntesis para no perder esa precedencia.
fn push_fragment(trees: &[TokenTree], kind: FragmentKind, output: &mut Vec<TokenTree>) {
    let (Some(first), Some(last), true) = (trees.first(), trees.last(), trees.len() > 1) else {
        output.extend(trees.iter().cloned());
        return;
    };
    let delimiter = if kind == FragmentKind::Expr {
        Delimiter::Parenthesis
    } else {
        Delimiter::Invisible
    };
    output.push(TokenTree::Group(Group {
        delimiter,
        open: first.first_origin(),
        close: last.last_origin(),
        trees: trees.to_vec(),
    }));
}

pub fn render(trees: &[TokenTree], rendered: &mut Rendered) {
    for tree in trees {
        match tree {
            TokenTree::Leaf(leaf) => rendered.piece(&leaf.text, leaf.origin),
            TokenTree::Group(group) if group.delimiter == Delimiter::Invisible => {
                render(&group.trees, rendered);
            }
            TokenTree::Group(group) => {
                let (open, close) = group.delimiter.marks();
                rendered.piece(open, group.open);
                render(&group.trees, rendered);
                rendered.piece(close, group.close);
            }
        }
    }
}

impl Rendered {
    fn piece(&mut self, text: &str, origin: Origin) {
        if !self.text.is_empty() {
            self.text.push(SPACE);
        }
        let start = self.text.len();
        self.text.push_str(text);
        self.spans.push(Span {
            start,
            end: self.text.len(),
            origin,
        });
    }
}
