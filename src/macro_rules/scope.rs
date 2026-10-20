use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use super::definition::Definition;
use super::expansion::Expansion;
use super::reader::definitions_in_text;
use super::registry::{CrateMacros, RecordedInvocation};
use super::tokens::Origin;

const LINE_BREAK: u8 = b'\n';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpansionKey {
    Written { source: usize, offset: usize },
    Expanded { expansion: usize, offset: usize },
}

#[derive(Debug)]
pub struct MacroScope<'scope> {
    crate_macros: Option<Arc<CrateMacros>>,
    current: usize,
    current_text: &'scope [u8],
    current_lines: Vec<usize>,
    local: Vec<Definition>,
    expansions: RefCell<HashMap<ExpansionKey, Option<Rc<Expansion>>>>,
}

impl<'scope> MacroScope<'scope> {
    pub fn new(
        crate_macros: Option<Arc<CrateMacros>>,
        path: Option<&Path>,
        current_text: &'scope [u8],
    ) -> Self {
        let current = crate_macros.as_ref().map_or(0, |macros| {
            path.and_then(|path| macros.source_of(path))
                .unwrap_or_else(|| macros.source_count())
        });
        let shares = crate_macros
            .as_ref()
            .zip(path)
            .is_none_or(|(macros, path)| macros.file_shares_macros(path));
        Self {
            local: definitions_in_text(
                std::str::from_utf8(current_text).unwrap_or_default(),
                current,
                shares,
            ),
            crate_macros,
            current,
            current_text,
            current_lines: line_starts(current_text),
            expansions: RefCell::new(HashMap::new()),
        }
    }

    pub const fn current(&self) -> usize {
        self.current
    }

    pub const fn current_text(&self) -> &'scope [u8] {
        self.current_text
    }

    pub fn source_text(&self, source: usize) -> Option<&[u8]> {
        if source == self.current {
            return Some(self.current_text);
        }
        self.crate_macros.as_ref()?.source_text(source)
    }

    pub fn position_in_current(&self, origin: Origin) -> Option<(usize, usize)> {
        (origin.source == self.current).then_some(())?;
        let row = self
            .current_lines
            .partition_point(|&start| start <= origin.offset)
            .checked_sub(1)?;
        let column = origin.offset.saturating_sub(*self.current_lines.get(row)?);
        Some((row, column))
    }

    // La misma invocación del archivo la piden la complejidad, el largo y el anidamiento: se expande
    // una sola vez.
    pub fn remembered(
        &self,
        key: ExpansionKey,
        expand: impl FnOnce() -> Option<Expansion>,
    ) -> Option<Rc<Expansion>> {
        if let Some(known) = self.expansions.borrow().get(&key) {
            return known.clone();
        }
        let expanded = expand().map(Rc::new);
        self.expansions.borrow_mut().insert(key, expanded.clone());
        expanded
    }

    // Una macro se busca por nombre como hace rustc con el alcance textual: primero la última
    // definición anterior que se ve desde ahí en el mismo archivo, después las demás de ese archivo
    // y al final las que el resto del crate comparte (las de `#[macro_use]` o `#[macro_export]`).
    pub fn candidates(&self, name: &str, at: Origin) -> Vec<&Definition> {
        let named = self.named(name);
        let mut preceding: Vec<&Definition> = named
            .iter()
            .copied()
            .filter(|definition| definition.is_visible_at(at) && definition.origin.offset < at.offset)
            .collect();
        preceding.sort_by_key(|definition| std::cmp::Reverse(definition.origin.offset));
        let following = named
            .iter()
            .copied()
            .filter(|definition| definition.is_visible_at(at) && definition.origin.offset >= at.offset);
        let elsewhere = named
            .iter()
            .copied()
            .filter(|definition| definition.origin.source != at.source && definition.shared);
        preceding.into_iter().chain(following).chain(elsewhere).collect()
    }

    fn named(&self, name: &str) -> Vec<&Definition> {
        let foreign = self
            .crate_macros
            .as_ref()
            .map(|macros| macros.definitions_named(name))
            .unwrap_or_default()
            .iter()
            .map(Arc::as_ref)
            .filter(|definition| definition.origin.source != self.current);
        self.local
            .iter()
            .filter(|definition| definition.name == name)
            .chain(foreign)
            .collect()
    }

    // Lo que una macro definida en este archivo arma con su propio texto (una función entera, por
    // ejemplo) clippy lo marca acá aunque la macro se use en otro archivo: hacen falta también las
    // invocaciones del resto del crate que llegan, directa o indirectamente, a estas macros.
    pub fn foreign_invocations(&self) -> Vec<&RecordedInvocation> {
        let Some(macros) = self.crate_macros.as_ref() else {
            return Vec::new();
        };
        if !self
            .local
            .iter()
            .any(|definition| definition.shared && definition.writes_functions)
        {
            return Vec::new();
        }
        let reaching = self.macros_reaching_current(macros);
        macros
            .invocations()
            .iter()
            .filter(|invocation| {
                invocation.source != self.current && reaching.contains(&invocation.name.as_str())
            })
            .collect()
    }

    fn macros_reaching_current<'macros>(&'macros self, macros: &'macros CrateMacros) -> Vec<&'macros str> {
        let mut reaching: Vec<&str> = self
            .local
            .iter()
            .filter(|definition| definition.shared && definition.writes_functions)
            .map(|definition| definition.name.as_str())
            .collect();
        let all: Vec<&Definition> = macros
            .all_definitions()
            .map(Arc::as_ref)
            .filter(|definition| definition.origin.source != self.current)
            .chain(self.local.iter())
            .filter(|definition| definition.shared)
            .collect();
        loop {
            let newly: Vec<&str> = all
                .iter()
                .filter(|definition| {
                    !reaching.contains(&definition.name.as_str())
                        && definition
                            .mentions
                            .iter()
                            .any(|mentioned| reaching.contains(&mentioned.as_str()))
                })
                .map(|definition| definition.name.as_str())
                .collect();
            if newly.is_empty() {
                return reaching;
            }
            reaching.extend(newly);
        }
    }
}

fn line_starts(text: &[u8]) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            text.iter()
                .enumerate()
                .filter(|&(_, &byte)| byte == LINE_BREAK)
                .map(|(index, _)| index.saturating_add(1)),
        )
        .collect()
}
