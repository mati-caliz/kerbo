use std::collections::HashSet;
use std::env::consts;

use tree_sitter::Node;

use crate::config::RustTarget;
use crate::macro_rules::Code;

const ATTRIBUTE: &str = "attribute_item";
const INNER_ATTRIBUTE: &str = "inner_attribute_item";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const MATCH_ARM: &str = "match_arm";
const CFG_PREFIX: &str = "cfg(";
const ATTRIBUTE_OPEN: &str = "[";
const INNER_MARK: &str = "!";
const HASH: &str = "#";
const CLOSE: char = ')';
const SEPARATOR: char = ',';
const EQUALS: char = '=';
const QUOTE: char = '"';
const ALL: &str = "all";
const ANY: &str = "any";
const NOT: &str = "not";
const FEATURE: &str = "feature";
const TEST: &str = "test";
const ALWAYS_TRUE: &[&str] = &["debug_assertions"];
const ALWAYS_FALSE: &[&str] = &["miri", "doc", "doctest"];
const ATOMIC_WIDTHS: &[&str] = &["8", "16", "32", "64", "ptr"];
const PANIC_STRATEGY: &str = "unwind";
const LITTLE_ENDIAN: &str = "little";
const BIG_ENDIAN: &str = "big";
const UNIX: &str = "unix";
const WINDOWS: &str = "windows";
const OPEN: char = '(';
const TARGET_OS: &str = "target_os";
const TARGET_FAMILY: &str = "target_family";
const TARGET_ARCH: &str = "target_arch";
const TARGET_POINTER_WIDTH: &str = "target_pointer_width";
const TARGET_ENDIAN: &str = "target_endian";
const TARGET_HAS_ATOMIC: &str = "target_has_atomic";
const PANIC: &str = "panic";

// clippy mide lo que compila: un ítem, una sentencia o un brazo con un `#[cfg]` falso no existe para
// la complejidad ni para el anidamiento. Se evalúa con la máquina donde corre kerbo, las features
// del Cargo.toml (todas, como `--all-features`) y `test` en los dos modos (la biblioteca y sus
// tests). Lo que no se conoce (un `cfg` que emite el build.rs, un `target_feature`) no decide: el
// código se mide, porque esconderlo por adivinar sería peor.
pub(in crate::metrics) fn is_compiled_out(node: Node<'_>, code: &Code<'_>) -> bool {
    let facts = facts_of(code);
    attributes_of(node)
        .into_iter()
        .filter_map(|attribute| predicate_text(attribute, code.bytes()))
        .any(|predicate| is_false(predicate, &facts))
}

pub(in crate::metrics) fn file_is_compiled_out(root: Node<'_>, code: &Code<'_>) -> bool {
    let facts = facts_of(code);
    let mut cursor = root.walk();
    root.children(&mut cursor)
        .filter(|child| child.kind() == INNER_ATTRIBUTE)
        .filter_map(|attribute| predicate_text(attribute, code.bytes()))
        .any(|predicate| is_false(predicate, &facts))
}

#[derive(Debug, Clone, Copy, Default)]
struct Facts<'facts> {
    features: Option<&'facts HashSet<String>>,
    target: Option<&'facts RustTarget>,
}

fn facts_of<'code>(code: &'code Code<'_>) -> Facts<'code> {
    Facts {
        features: code.scope().crate_features(),
        target: code.scope().target(),
    }
}

fn attributes_of(node: Node<'_>) -> Vec<Node<'_>> {
    let mut attributes = Vec::new();
    let mut previous = node.prev_sibling();
    while let Some(sibling) = previous {
        match sibling.kind() {
            ATTRIBUTE => attributes.push(sibling),
            kind if COMMENTS.contains(&kind) => {}
            _ => break,
        }
        previous = sibling.prev_sibling();
    }
    if node.kind() == MATCH_ARM {
        let mut cursor = node.walk();
        attributes.extend(
            node.children(&mut cursor)
                .filter(|child| child.kind() == ATTRIBUTE),
        );
    }
    attributes
}

fn predicate_text<'source>(attribute: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let text = attribute.utf8_text(source).ok()?;
    let inside = text
        .strip_prefix(HASH)?
        .trim_start()
        .trim_start_matches(INNER_MARK)
        .trim_start()
        .strip_prefix(ATTRIBUTE_OPEN)?
        .trim_start();
    let predicate = inside.strip_prefix(CFG_PREFIX)?;
    let end = predicate.rfind(CLOSE)?;
    predicate.get(..end)
}

fn is_false(predicate: &str, facts: &Facts<'_>) -> bool {
    [false, true].into_iter().all(|test| {
        let mut parser = Predicate {
            rest: predicate,
            facts: *facts,
            test,
        };
        parser.value() == Some(false)
    })
}

struct Predicate<'text> {
    rest: &'text str,
    facts: Facts<'text>,
    test: bool,
}

impl<'text> Predicate<'text> {
    fn value(&mut self) -> Option<bool> {
        let name = self.word()?;
        if self.take(OPEN) {
            let values = self.list();
            return match name {
                ALL => all(&values),
                ANY => any(&values),
                NOT => values.first().copied().flatten().map(|value| !value),
                _ => None,
            };
        }
        if self.take(EQUALS) {
            let value = self.string()?;
            return self.keyed(name, value);
        }
        self.named(name)
    }

    fn list(&mut self) -> Vec<Option<bool>> {
        let mut values = Vec::new();
        loop {
            self.skip_spaces();
            if self.take(CLOSE) || self.rest.is_empty() {
                return values;
            }
            if self.take(SEPARATOR) {
                continue;
            }
            let before = self.rest.len();
            values.push(self.value());
            if self.rest.len() == before {
                return values;
            }
        }
    }

    fn named(&self, name: &str) -> Option<bool> {
        match name {
            TEST => Some(self.test),
            UNIX | WINDOWS => Some(self.family() == name),
            _ if ALWAYS_TRUE.contains(&name) => Some(true),
            _ if ALWAYS_FALSE.contains(&name) => Some(false),
            _ => self.extra(name),
        }
    }

    fn keyed(&self, name: &str, value: &str) -> Option<bool> {
        let target = self.facts.target;
        let host = match name {
            FEATURE => return self.feature(value),
            TARGET_OS => target
                .and_then(|target| target.target_os.as_deref())
                .unwrap_or(consts::OS),
            TARGET_FAMILY => self.family(),
            TARGET_ARCH => target
                .and_then(|target| target.target_arch.as_deref())
                .unwrap_or(consts::ARCH),
            TARGET_POINTER_WIDTH => return Some(value == usize::BITS.to_string()),
            TARGET_ENDIAN => {
                if cfg!(target_endian = "little") {
                    LITTLE_ENDIAN
                } else {
                    BIG_ENDIAN
                }
            }
            TARGET_HAS_ATOMIC => return Some(ATOMIC_WIDTHS.contains(&value)),
            PANIC => PANIC_STRATEGY,
            _ => return self.extra(&format!("{name}{EQUALS}{QUOTE}{value}{QUOTE}")),
        };
        Some(value == host)
    }

    fn feature(&self, value: &str) -> Option<bool> {
        let chosen = self.facts.target.and_then(|target| target.features.as_ref());
        chosen.map_or_else(
            || self.facts.features.map(|features| features.contains(value)),
            |chosen| Some(chosen.iter().any(|feature| feature == value)),
        )
    }

    // Con `target_os` en kerbo.toml, la familia sale de ahí: Windows es `windows` y el resto `unix`.
    fn family(&self) -> &'static str {
        match self.facts.target.and_then(|target| target.target_os.as_deref()) {
            Some(WINDOWS) => WINDOWS,
            Some(_) => UNIX,
            None => consts::FAMILY,
        }
    }

    // Un `cfg` que no es del compilador lo enciende el build.rs: si kerbo.toml lo nombra, vale; si
    // no, no se sabe.
    fn extra(&self, written: &str) -> Option<bool> {
        let compact: String = written
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        self.facts
            .target
            .is_some_and(|target| {
                target
                    .cfg
                    .iter()
                    .any(|enabled| enabled.replace(' ', "") == compact)
            })
            .then_some(true)
    }

    fn word(&mut self) -> Option<&'text str> {
        self.skip_spaces();
        let end = self
            .rest
            .find(|character: char| !(character.is_alphanumeric() || character == '_'))
            .unwrap_or(self.rest.len());
        let (word, rest) = self.rest.split_at(end);
        self.rest = rest;
        (!word.is_empty()).then_some(word)
    }

    fn string(&mut self) -> Option<&'text str> {
        self.skip_spaces();
        let body = self.rest.strip_prefix(QUOTE)?;
        let end = body.find(QUOTE)?;
        let (value, rest) = body.split_at(end);
        self.rest = rest.get(QUOTE.len_utf8()..).unwrap_or_default();
        Some(value)
    }

    fn take(&mut self, expected: char) -> bool {
        self.skip_spaces();
        match self.rest.strip_prefix(expected) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn skip_spaces(&mut self) {
        self.rest = self.rest.trim_start();
    }
}

fn all(values: &[Option<bool>]) -> Option<bool> {
    if values.contains(&Some(false)) {
        return Some(false);
    }
    values.iter().all(|value| *value == Some(true)).then_some(true)
}

fn any(values: &[Option<bool>]) -> Option<bool> {
    if values.contains(&Some(true)) {
        return Some(true);
    }
    values.iter().all(|value| *value == Some(false)).then_some(false)
}

#[cfg(test)]
mod tests;
