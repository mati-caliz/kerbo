use std::sync::LazyLock;

use regex::Regex;

const GLOB: &str = "*";

static USE_DECLARATION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\buse\s+(?:::)?([A-Za-z_][A-Za-z0-9_]*)::([^;]*);").ok());
static MACRO_USE_CRATE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"#\[macro_use\]\s*extern\s+crate\s+([A-Za-z_][A-Za-z0-9_]*)").ok());

// De qué crate puede venir una macro que el archivo invoca sin ruta: la nombra un `use dep::...`,
// la trae un `use dep::*` o la deja un `#[macro_use] extern crate dep`.
#[derive(Debug, Default)]
pub struct Imports {
    named: Vec<(String, Vec<String>)>,
    everything: Vec<String>,
}

impl Imports {
    pub fn of(text: &str) -> Self {
        let mut imports = Self::default();
        for (krate, rest) in use_declarations(text) {
            if rest.trim() == GLOB {
                imports.everything.push(krate.to_owned());
            } else {
                imports.named.push((krate.to_owned(), words(rest)));
            }
        }
        if let Some(pattern) = MACRO_USE_CRATE.as_ref() {
            imports.everything.extend(
                pattern
                    .captures_iter(text)
                    .filter_map(|found| found.get(1).map(|krate| krate.as_str().to_owned())),
            );
        }
        imports
    }

    pub fn crates_offering<'imports>(
        &'imports self,
        name: &'imports str,
    ) -> impl Iterator<Item = &'imports str> {
        self.named
            .iter()
            .filter(move |(_, words)| words.iter().any(|word| word == name))
            .map(|(krate, _)| krate.as_str())
            .chain(self.everything.iter().map(String::as_str))
    }
}

fn use_declarations(text: &str) -> Vec<(&str, &str)> {
    USE_DECLARATION
        .as_ref()
        .map(|pattern| {
            pattern
                .captures_iter(text)
                .filter_map(|found| Some((found.get(1)?.as_str(), found.get(2)?.as_str())))
                .collect()
        })
        .unwrap_or_default()
}

fn words(path: &str) -> Vec<String> {
    path.split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}
