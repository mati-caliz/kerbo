use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::definition::{Definition, mentioned_macros, parse_rules, writes_functions};
use super::tokens::{Delimiter, Group, LeafKind, Origin, TokenTree, lex};

const DEFINITION_MARKER: &str = "macro_rules!";
const DEFINITION_KEYWORD: &str = "macro_rules";
const BANG: &str = "!";
const HASH: &str = "#";
const EXPORT_ATTRIBUTE: &str = "macro_export";
const MODULE_FILE: &str = "mod";
const CRATE_ROOTS: &[&str] = &["lib", "main"];
const TARGET_DIRECTORIES: &[&str] = &["tests", "examples", "benches", "bin"];
const ATTRIBUTE_LENGTH: usize = 2;
const DEFINITION_TOKENS: usize = 4;
static MACRO_USE_MODULE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"#\[macro_use\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)")
        .ok()
});

#[derive(Debug, Clone, Copy)]
struct Place {
    source: usize,
    shares: bool,
    scope: Option<(usize, usize)>,
}

pub fn macro_use_modules<'text>(texts: impl Iterator<Item = &'text str>) -> HashSet<String> {
    let Some(pattern) = MACRO_USE_MODULE.as_ref() else {
        return HashSet::new();
    };
    texts
        .flat_map(|text| {
            pattern
                .captures_iter(text)
                .filter_map(|found| found.get(1).map(|name| name.as_str().to_owned()))
        })
        .collect()
}

// Las macros de un archivo llegan a los demás si el archivo es la raíz de un crate (`lib.rs`,
// `main.rs`, un test o un ejemplo) o si su módulo se declara con `#[macro_use]`; las que no, quedan
// en su archivo, como el alcance textual de rustc.
pub fn file_shares_macros(path: &Path, macro_use: &HashSet<String>) -> bool {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let parent_name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let module = if stem == MODULE_FILE { parent_name } else { stem };
    CRATE_ROOTS.contains(&stem) || TARGET_DIRECTORIES.contains(&parent_name) || macro_use.contains(module)
}

// Las definiciones se leen con el tokenizador propio y no con tree-sitter: alcanza con los grupos
// de llaves para saber dónde se ve cada una, y leer así todo el crate es mucho más barato.
pub fn definitions_in_text(text: &str, source: usize, shares: bool) -> Vec<Definition> {
    if !text.contains(DEFINITION_MARKER) {
        return Vec::new();
    }
    let locate = |offset: usize| Origin {
        source,
        offset,
        expanded: true,
    };
    let Some(trees) = lex(text, &locate) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let place = Place {
        source,
        shares,
        scope: None,
    };
    collect_definitions(&trees, place, &mut found);
    found
}

fn collect_definitions(trees: &[TokenTree], place: Place, found: &mut Vec<Definition>) {
    let mut index = 0;
    while let Some(tree) = trees.get(index) {
        if let Some(definition) = definition_at(trees, index, place) {
            found.push(definition);
            index = index.saturating_add(DEFINITION_TOKENS);
            continue;
        }
        if let Some(group) = tree.group().filter(|group| is_scope(trees, index, group)) {
            let scope = Some((group.open.offset, group.close.offset.saturating_add(1)));
            collect_definitions(&group.trees, Place { scope, ..place }, found);
        }
        index = index.saturating_add(1);
    }
}

fn is_scope(trees: &[TokenTree], index: usize, group: &Group) -> bool {
    let after_bang = index
        .checked_sub(1)
        .and_then(|previous| trees.get(previous))
        .is_some_and(|previous| previous.is_punct(BANG));
    group.delimiter == Delimiter::Brace && !after_bang
}

fn definition_at(trees: &[TokenTree], index: usize, place: Place) -> Option<Definition> {
    let [keyword, bang, name, body, ..] = trees.get(index..)? else {
        return None;
    };
    let name = name.leaf().filter(|leaf| leaf.kind == LeafKind::Ident)?;
    if !keyword.is_word(DEFINITION_KEYWORD) || !bang.is_punct(BANG) {
        return None;
    }
    let body = body.group()?;
    let rules = parse_rules(&body.trees)?;
    let top_level = place.scope.is_none();
    Some(Definition {
        name: name.text.clone(),
        mentions: mentioned_macros(&rules),
        writes_functions: writes_functions(&rules),
        rules,
        origin: Origin {
            source: place.source,
            offset: body.open.offset,
            expanded: true,
        },
        visible_in: place.scope,
        shared: is_exported(trees, index) || (top_level && place.shares),
    })
}

fn is_exported(trees: &[TokenTree], index: usize) -> bool {
    let mut end = index;
    while let Some(start) = end.checked_sub(ATTRIBUTE_LENGTH) {
        let (Some(hash), Some(TokenTree::Group(attribute))) =
            (trees.get(start), trees.get(start.saturating_add(1)))
        else {
            return false;
        };
        if !hash.is_punct(HASH) || attribute.delimiter != Delimiter::Bracket {
            return false;
        }
        if attribute.trees.iter().any(|tree| tree.is_word(EXPORT_ATTRIBUTE)) {
            return true;
        }
        end = start;
    }
    false
}
