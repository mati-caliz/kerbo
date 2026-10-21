use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use super::definition::Definition;
use super::dependencies::Dependency;
use super::reader::definitions_in_text;
use super::registry::rust_files;

const DEFINITION_MARKER: &str = "macro_rules!";
const FOREIGN_SOURCE_BASE: usize = 1 << 24;
const FOREIGN_SOURCE_STRIDE: usize = 1 << 16;

// Las macros de una dependencia: se leen recién cuando una invocación la nombra, porque la mayoría de
// las dependencias de un crate no aporta ninguna. Sus archivos usan números de fuente propios, lejos
// de los del crate, así lo que sale de una expansión se sabe de dónde vino.
#[derive(Debug)]
pub struct DependencyMacros {
    name: String,
    root: PathBuf,
    first_source: usize,
    loaded: OnceLock<Loaded>,
}

#[derive(Debug, Default)]
struct Loaded {
    texts: HashMap<usize, Arc<str>>,
    definitions: HashMap<String, Vec<Arc<Definition>>>,
}

impl DependencyMacros {
    pub fn new(dependency: Dependency, position: usize) -> Self {
        Self {
            name: dependency.name,
            root: dependency.root,
            first_source: FOREIGN_SOURCE_BASE.saturating_add(position.saturating_mul(FOREIGN_SOURCE_STRIDE)),
            loaded: OnceLock::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn owns(&self, source: usize) -> bool {
        (self.first_source..self.first_source.saturating_add(FOREIGN_SOURCE_STRIDE)).contains(&source)
    }

    pub fn definitions_named(&self, name: &str) -> &[Arc<Definition>] {
        self.loaded().definitions.get(name).map_or(&[], Vec::as_slice)
    }

    pub fn source_text(&self, source: usize) -> Option<&[u8]> {
        self.loaded().texts.get(&source).map(|text| text.as_bytes())
    }

    pub fn hash_identity<H: Hasher>(&self, hasher: &mut H) {
        self.name.hash(hasher);
        self.root.hash(hasher);
    }

    fn loaded(&self) -> &Loaded {
        self.loaded.get_or_init(|| self.load())
    }

    fn load(&self) -> Loaded {
        let mut loaded = Loaded::default();
        for (position, path) in rust_files(&self.root)
            .into_iter()
            .enumerate()
            .take(FOREIGN_SOURCE_STRIDE)
        {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if !text.contains(DEFINITION_MARKER) {
                continue;
            }
            let source = self.first_source.saturating_add(position);
            for definition in definitions_in_text(&text, source, false) {
                loaded
                    .definitions
                    .entry(definition.name.clone())
                    .or_default()
                    .push(Arc::new(definition));
            }
            loaded.texts.insert(source, Arc::from(text));
        }
        loaded
    }
}
