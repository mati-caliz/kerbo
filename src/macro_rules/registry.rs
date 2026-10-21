use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError};
use std::time::SystemTime;

use ignore::WalkBuilder;
use tree_sitter::Node;

use super::MacroPath;
use super::definition::Definition;
use super::dependencies::{declared_features, dependencies};
use super::foreign::DependencyMacros;
use super::reader::{definitions_in_text, file_shares_macros, macro_use_modules};
use crate::config::RustTarget;
use crate::language::Language;
use crate::syntax::{SnippetLanguage, descendants, parse_snippet};

const MANIFEST: &str = "Cargo.toml";
const RUST_EXTENSION: &str = "rs";
const TARGET_DIRECTORY: &str = "target";
const INVOCATION_KIND: &str = "macro_invocation";
const MACRO_FIELD: &str = "macro";
const TOKEN_TREE: &str = "token_tree";
const BANG: char = '!';
const DEFINITION_MARKER: &str = "macro_rules!";
const MACRO_USE_MARKER: &str = "macro_use";

#[derive(Debug)]
struct SourceText {
    path: PathBuf,
    text: Arc<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Statements,
    Items,
    ImplItems { implements_trait: bool },
    TraitItems,
}

#[derive(Debug, Clone)]
pub struct RecordedInvocation {
    pub source: usize,
    pub name: String,
    pub arguments: (usize, usize),
    pub context: Context,
}

type Fingerprint = Vec<(PathBuf, u64, Option<SystemTime>)>;

#[derive(Debug)]
pub struct CrateMacros {
    fingerprint: Fingerprint,
    root: PathBuf,
    content_hash: u64,
    sources: Vec<SourceText>,
    definitions: HashMap<String, Vec<Arc<Definition>>>,
    macro_use: HashSet<String>,
    dependencies: Vec<DependencyMacros>,
    features: HashSet<String>,
    target: RustTarget,
    invocations: OnceLock<Vec<RecordedInvocation>>,
}

impl CrateMacros {
    pub fn definitions_named(&self, name: &str) -> &[Arc<Definition>] {
        self.definitions.get(name).map_or(&[], Vec::as_slice)
    }

    pub fn all_definitions(&self) -> impl Iterator<Item = &Arc<Definition>> {
        self.definitions.values().flatten()
    }

    pub const fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn source_text(&self, source: usize) -> Option<&[u8]> {
        self.sources.get(source).map_or_else(
            || self.dependency_of_source(source)?.source_text(source),
            |text| Some(text.text.as_bytes()),
        )
    }

    pub const fn features(&self) -> &HashSet<String> {
        &self.features
    }

    pub const fn target(&self) -> &RustTarget {
        &self.target
    }

    pub fn dependency_named(&self, name: &str) -> Option<&DependencyMacros> {
        self.dependencies
            .iter()
            .find(|dependency| dependency.name() == name)
    }

    pub fn dependency_of_source(&self, source: usize) -> Option<&DependencyMacros> {
        self.dependencies
            .iter()
            .find(|dependency| dependency.owns(source))
    }

    pub fn file_shares_macros(&self, path: &Path) -> bool {
        file_shares_macros(path, &self.macro_use)
    }

    pub fn source_of(&self, path: &Path) -> Option<usize> {
        let wanted = canonical(path);
        self.sources.iter().position(|source| source.path == wanted)
    }

    pub fn invocations(&self) -> &[RecordedInvocation] {
        self.invocations.get_or_init(|| record_invocations(self))
    }

    pub fn analysis_context(&self, file: &Path) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.content_hash.hash(&mut hasher);
        self.target.hash(&mut hasher);
        relative_to(&self.root, &canonical(file)).hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Debug)]
pub struct MacroIndex {
    verifies_freshness: bool,
    crates: Mutex<HashMap<PathBuf, CrateCell>>,
}

type CrateCell = Arc<OnceLock<Arc<CrateMacros>>>;

static SHARED: LazyLock<MacroIndex> = LazyLock::new(|| MacroIndex {
    verifies_freshness: true,
    crates: Mutex::new(HashMap::new()),
});

impl MacroIndex {
    pub fn for_one_scan() -> Self {
        Self {
            verifies_freshness: false,
            crates: Mutex::new(HashMap::new()),
        }
    }

    pub fn shared() -> &'static Self {
        &SHARED
    }

    pub fn crate_with(
        &self,
        language: Language,
        file: &Path,
        target: &RustTarget,
    ) -> Option<Arc<CrateMacros>> {
        (language == Language::Rust)
            .then(|| self.crate_for(file, target))
            .flatten()
    }

    // Cada crate se lee una sola vez aunque lo pidan varios hilos a la vez: los demás esperan la
    // misma celda.
    fn crate_for(&self, file: &Path, target: &RustTarget) -> Option<Arc<CrateMacros>> {
        let root = crate_root(&canonical(file))?;
        let cell = Arc::clone(
            self.crates
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(root.clone())
                .or_default(),
        );
        let known = cell.get_or_init(|| Arc::new(collect(&root, target)));
        let same_target = known.target == *target;
        if same_target && (!self.verifies_freshness || known.fingerprint == fingerprint(&rust_files(&root))) {
            return Some(Arc::clone(known));
        }
        let fresh = Arc::new(collect(&root, target));
        self.crates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(root, Arc::new(OnceLock::from(Arc::clone(&fresh))));
        Some(fresh)
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn relative_to<'path>(root: &Path, path: &'path Path) -> &'path Path {
    path.strip_prefix(root).unwrap_or(path)
}

fn crate_root(file: &Path) -> Option<PathBuf> {
    file.ancestors()
        .skip(1)
        .find(|directory| directory.join(MANIFEST).is_file())
        .map(Path::to_path_buf)
}

pub(super) fn rust_files(root: &Path) -> Vec<PathBuf> {
    let crate_root = root.to_path_buf();
    let mut files: Vec<PathBuf> = WalkBuilder::new(root)
        .filter_entry(move |entry| {
            let path = entry.path();
            let is_directory = entry.file_type().is_some_and(|kind| kind.is_dir());
            !is_directory
                || path == crate_root
                || !(path.join(MANIFEST).is_file() || entry.file_name() == TARGET_DIRECTORY)
        })
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .map(|entry| canonical(entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == RUST_EXTENSION)
        })
        .collect();
    files.sort();
    files
}

fn fingerprint(files: &[PathBuf]) -> Fingerprint {
    files
        .iter()
        .map(|path| {
            let metadata = fs::metadata(path).ok();
            (
                path.clone(),
                metadata.as_ref().map_or(0, fs::Metadata::len),
                metadata.and_then(|metadata| metadata.modified().ok()),
            )
        })
        .collect()
}

fn collect(root: &Path, target: &RustTarget) -> CrateMacros {
    let files = rust_files(root);
    let sources: Vec<SourceText> = files
        .iter()
        .filter_map(|path| {
            fs::read_to_string(path).ok().map(|text| SourceText {
                path: path.clone(),
                text: Arc::from(text),
            })
        })
        .collect();
    let macro_use = macro_use_modules(sources.iter().map(|source| source.text.as_ref()));
    let mut definitions: HashMap<String, Vec<Arc<Definition>>> = HashMap::new();
    for (index, source) in sources.iter().enumerate() {
        let shares = file_shares_macros(&source.path, &macro_use);
        for definition in definitions_in_text(&source.text, index, shares) {
            definitions
                .entry(definition.name.clone())
                .or_default()
                .push(Arc::new(definition));
        }
    }
    let mut macros = CrateMacros {
        fingerprint: fingerprint(&files),
        root: root.to_path_buf(),
        content_hash: 0,
        sources,
        definitions,
        macro_use,
        dependencies: dependencies(root)
            .into_iter()
            .enumerate()
            .map(|(position, dependency)| DependencyMacros::new(dependency, position))
            .collect(),
        features: declared_features(root),
        target: target.clone(),
        invocations: OnceLock::new(),
    };
    macros.content_hash = macro_text_hash(&macros);
    macros
}

// Lo único de otro archivo que cambia el análisis de uno son las macros: las definiciones, los
// `#[macro_use]` y las invocaciones de las que arman funciones. Entra la lista de archivos y el
// texto entero sólo de los que tienen alguna de esas tres cosas; editar el cuerpo de una función
// común no invalida el resto del crate en la caché.
fn macro_text_hash(macros: &CrateMacros) -> u64 {
    let writer_marks: Vec<String> = function_writers(macros)
        .iter()
        .map(|name| format!("{name}{BANG}"))
        .collect();
    let mut hasher = DefaultHasher::new();
    for source in &macros.sources {
        relative_to(&macros.root, &source.path).hash(&mut hasher);
        let text = source.text.as_ref();
        let shapes_others = text.contains(DEFINITION_MARKER)
            || text.contains(MACRO_USE_MARKER)
            || writer_marks.iter().any(|mark| text.contains(mark.as_str()));
        if shapes_others {
            text.hash(&mut hasher);
        }
    }
    for dependency in &macros.dependencies {
        dependency.hash_identity(&mut hasher);
    }
    hasher.finish()
}

fn record_invocations(macros: &CrateMacros) -> Vec<RecordedInvocation> {
    let marks: Vec<String> = function_writers(macros)
        .iter()
        .map(|name| format!("{name}{BANG}"))
        .collect();
    macros
        .sources
        .iter()
        .enumerate()
        .filter(|(_, source)| marks.iter().any(|mark| source.text.contains(mark.as_str())))
        .flat_map(|(index, source)| invocations_in_source(macros, index, &source.text))
        .collect()
}

// Sólo hacen falta las invocaciones que terminan armando una función con texto de la definición,
// directamente o a través de otras macros.
fn function_writers(macros: &CrateMacros) -> HashSet<&str> {
    let shared: Vec<&Definition> = macros
        .all_definitions()
        .map(Arc::as_ref)
        .filter(|definition| definition.shared)
        .collect();
    let mut writers: HashSet<&str> = shared
        .iter()
        .filter(|definition| definition.writes_functions)
        .map(|definition| definition.name.as_str())
        .collect();
    loop {
        let newly: Vec<&str> = shared
            .iter()
            .filter(|definition| {
                !writers.contains(definition.name.as_str())
                    && definition
                        .mentions
                        .iter()
                        .any(|mentioned| writers.contains(mentioned.as_str()))
            })
            .map(|definition| definition.name.as_str())
            .collect();
        if newly.is_empty() {
            return writers;
        }
        writers.extend(newly);
    }
}

fn invocations_in_source(macros: &CrateMacros, source: usize, text: &str) -> Vec<RecordedInvocation> {
    let Some(tree) = parse_snippet(SnippetLanguage::Rust, text) else {
        return Vec::new();
    };
    let bytes = text.as_bytes();
    descendants(tree.root_node())
        .into_iter()
        .filter(|node| node.kind() == INVOCATION_KIND)
        .filter_map(|node| {
            let name = local_macro_name(node, bytes)?;
            macros.definitions.contains_key(name).then_some(())?;
            let arguments = arguments_range(node)?;
            Some(RecordedInvocation {
                source,
                name: name.to_owned(),
                arguments,
                context: super::context_of(node),
            })
        })
        .collect()
}

pub fn local_macro_name<'text>(invocation: Node<'_>, text: &'text [u8]) -> Option<&'text str> {
    super::local_name(invocation_path_text(invocation, text)?)
}

pub fn invocation_path<'text>(invocation: Node<'_>, text: &'text [u8]) -> Option<MacroPath<'text>> {
    super::macro_path(invocation_path_text(invocation, text)?)
}

fn invocation_path_text<'text>(invocation: Node<'_>, text: &'text [u8]) -> Option<&'text str> {
    invocation.child_by_field_name(MACRO_FIELD)?.utf8_text(text).ok()
}

pub fn arguments_range(invocation: Node<'_>) -> Option<(usize, usize)> {
    let mut cursor = invocation.walk();
    let tree = invocation
        .children(&mut cursor)
        .find(|child| child.kind() == TOKEN_TREE)?;
    Some((tree.start_byte(), tree.end_byte()))
}
