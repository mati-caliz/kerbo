use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tree_sitter::{Node, Tree};

use super::matcher::match_rule;
use super::registry::{Context, RecordedInvocation};
use super::scope::{ExpansionKey, MacroScope};
use super::tokens::{Origin, lex};
use super::transcriber::{Rendered, Span, render, transcribe};
use crate::syntax::{SnippetLanguage, parse_snippet};

const RECURSION_LIMIT: usize = 64;
const STATEMENTS_PREFIX: &str = "fn kerbo_expansion() {\n";
const IMPL_PREFIX: &str = "impl KerboExpansion {\n";
const TRAIT_IMPL_PREFIX: &str = "impl KerboTrait for KerboExpansion {\n";
const TRAIT_PREFIX: &str = "trait KerboExpansion {\n";
const BLOCK_SUFFIX: &str = "\n}";
const BODY_FIELD: &str = "body";

static NEXT_EXPANSION: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone)]
enum Base {
    Direct { source: usize, expanded: bool },
    Spans(Arc<[Span]>),
}

#[derive(Debug, Clone)]
struct OriginMap {
    base: Base,
    removed: usize,
    added: usize,
}

impl OriginMap {
    fn origin_at(&self, byte: usize) -> Option<Origin> {
        let position = byte.checked_sub(self.removed)?.saturating_add(self.added);
        match &self.base {
            Base::Direct { source, expanded } => Some(Origin {
                source: *source,
                offset: position,
                expanded: *expanded,
            }),
            Base::Spans(spans) => {
                let index = spans
                    .partition_point(|span| span.start <= position)
                    .checked_sub(1)?;
                let span = spans.get(index).filter(|span| position < span.end)?;
                Some(Origin {
                    offset: span
                        .origin
                        .offset
                        .saturating_add(position.saturating_sub(span.start)),
                    ..span.origin
                })
            }
        }
    }

    fn first_expanded(&self, start: usize, end: usize) -> Option<Origin> {
        let Base::Spans(spans) = &self.base else {
            return None;
        };
        let shift = |byte: usize| {
            byte.checked_sub(self.removed)
                .map(|byte| byte.saturating_add(self.added))
        };
        let (from, to) = (shift(start)?, shift(end)?);
        let first = spans.partition_point(|span| span.end <= from);
        spans
            .get(first..)?
            .iter()
            .take_while(|span| span.start < to)
            .find(|span| span.origin.expanded)
            .map(|span| span.origin)
    }
}

#[derive(Debug, Clone)]
pub struct Code<'code> {
    bytes: &'code [u8],
    map: OriginMap,
    scope: &'code MacroScope<'code>,
    depth: usize,
    expansion: Option<usize>,
}

impl<'code> Code<'code> {
    pub const fn bytes(&self) -> &'code [u8] {
        self.bytes
    }

    pub const fn scope(&self) -> &'code MacroScope<'code> {
        self.scope
    }

    pub const fn root(bytes: &'code [u8], scope: &'code MacroScope<'code>) -> Self {
        Self::of_source(bytes, scope.current(), scope)
    }

    pub const fn of_source(bytes: &'code [u8], source: usize, scope: &'code MacroScope<'code>) -> Self {
        Self {
            bytes,
            map: OriginMap {
                base: Base::Direct {
                    source,
                    expanded: false,
                },
                removed: 0,
                added: 0,
            },
            scope,
            depth: 0,
            expansion: None,
        }
    }

    // Un pedazo de este código compilado aparte: `prefix` bytes de envoltorio y después el texto
    // que arranca en `start`.
    pub fn snippet<'snippet>(&self, bytes: &'snippet [u8], prefix: usize, start: usize) -> Code<'snippet>
    where
        'code: 'snippet,
    {
        Code {
            bytes,
            map: OriginMap {
                base: self.map.base.clone(),
                removed: self.map.removed.saturating_add(prefix),
                added: self.map.added.saturating_add(start),
            },
            scope: self.scope,
            depth: self.depth,
            expansion: None,
        }
    }

    pub fn origin_at(&self, byte: usize) -> Option<Origin> {
        self.map.origin_at(byte)
    }

    pub fn is_written_here(&self, start: usize, last: usize) -> bool {
        [start, last].into_iter().all(|byte| {
            self.origin_at(byte)
                .is_some_and(|origin| !origin.expanded && origin.source == self.scope.current())
        })
    }

    pub fn position(&self, byte: usize) -> Option<(usize, usize)> {
        self.scope.position_in_current(self.origin_at(byte)?)
    }

    // Donde rustc ubica un ítem que arma una macro: si mezcla texto de la invocación con texto de
    // la definición, el span es el de la definición (aunque la visibilidad venga de afuera).
    pub fn item_position(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        self.map.first_expanded(start, end).map_or_else(
            || self.position(start),
            |origin| self.scope.position_in_current(origin),
        )
    }

    pub fn expand_node(&self, invocation: Node<'_>) -> Option<Rc<Expansion>> {
        let name = super::registry::local_macro_name(invocation, self.bytes)?;
        let arguments = super::registry::arguments_range(invocation)?;
        let context = super::context_of(invocation);
        let expand = || self.expand(name, arguments, context);
        self.expansion_key(arguments.0)
            .map_or_else(|| expand().map(Rc::new), |key| self.scope.remembered(key, expand))
    }

    pub fn expand_recorded(&self, invocation: &RecordedInvocation) -> Option<Expansion> {
        self.expand(&invocation.name, invocation.arguments, invocation.context)
    }

    fn expansion_key(&self, byte: usize) -> Option<ExpansionKey> {
        if let Some(expansion) = self.expansion {
            return Some(ExpansionKey::Expanded {
                expansion,
                offset: byte,
            });
        }
        let is_written = matches!(self.map.base, Base::Direct { expanded: false, .. }) && self.depth == 0;
        let origin = is_written.then(|| self.origin_at(byte)).flatten()?;
        Some(ExpansionKey::Written {
            source: origin.source,
            offset: origin.offset,
        })
    }

    fn expand(&self, name: &str, (start, end): (usize, usize), context: Context) -> Option<Expansion> {
        if self.depth >= RECURSION_LIMIT {
            return None;
        }
        let at = self.origin_at(start)?;
        let candidates = self.scope.candidates(name, at);
        if candidates.is_empty() {
            return None;
        }
        let text = std::str::from_utf8(self.bytes.get(start..end)?).ok()?;
        let locate = |offset: usize| self.origin_at(start.saturating_add(offset)).unwrap_or(at);
        let trees = lex(text, &locate)?;
        let input = &trees.first()?.group()?.trees;
        let output = candidates.into_iter().find_map(|definition| {
            definition
                .rules
                .iter()
                .find_map(|rule| match_rule(rule, input).map(|bindings| (rule, bindings)))
        })?;
        let (rule, bindings) = output;
        let transcribed = transcribe(&rule.transcriber, &bindings)?;
        let mut rendered = Rendered::default();
        render(&transcribed, &mut rendered);
        Expansion::parse(rendered, context, self.depth.saturating_add(1))
    }
}

#[derive(Debug)]
pub struct Expansion {
    id: usize,
    text: String,
    tree: Tree,
    spans: Arc<[Span]>,
    context: Context,
    depth: usize,
}

impl Expansion {
    fn parse(rendered: Rendered, context: Context, depth: usize) -> Option<Self> {
        let prefix = match context {
            Context::Statements => STATEMENTS_PREFIX,
            Context::Items => "",
            Context::ImplItems {
                implements_trait: false,
            } => IMPL_PREFIX,
            Context::ImplItems {
                implements_trait: true,
            } => TRAIT_IMPL_PREFIX,
            Context::TraitItems => TRAIT_PREFIX,
        };
        let suffix = if prefix.is_empty() { "" } else { BLOCK_SUFFIX };
        let text = format!("{prefix}{}{suffix}", rendered.text);
        let tree = parse_snippet(SnippetLanguage::Rust, &text)?;
        let spans: Arc<[Span]> = rendered
            .spans
            .into_iter()
            .map(|span| Span {
                start: span.start.saturating_add(prefix.len()),
                end: span.end.saturating_add(prefix.len()),
                origin: span.origin,
            })
            .collect();
        Some(Self {
            id: NEXT_EXPANSION.fetch_add(1, Ordering::Relaxed),
            text,
            tree,
            spans,
            context,
            depth,
        })
    }

    pub fn code<'code>(&'code self, scope: &'code MacroScope<'code>) -> Code<'code> {
        Code {
            bytes: self.text.as_bytes(),
            map: OriginMap {
                base: Base::Spans(Arc::clone(&self.spans)),
                removed: 0,
                added: 0,
            },
            scope,
            depth: self.depth,
            expansion: Some(self.id),
        }
    }

    pub fn content(&self) -> Option<Node<'_>> {
        let root = self.tree.root_node();
        if self.context == Context::Items {
            return Some(root);
        }
        root.named_child(0)?.child_by_field_name(BODY_FIELD)
    }
}
