use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::nodes::{COMMENT, parent_without_parentheses};
use super::scope::GLOBAL;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const KNOWN_GLOBALS: &str = include_str!("globals.txt");
const TYPESCRIPT_LIB: &str = include_str!("typescript_lib.txt");
const LIB_VALUE: &str = "value";
const LIB_SEPARATOR: char = ' ';
const HEADER_MARK: &str = "#";
const BLOCK_PREFIX: &str = "/*";
const BLOCK_SUFFIX: &str = "*/";
const VALUE_SEPARATOR: char = ':';
const NORMALIZED_VALUE: &str = ":";
const TURNED_OFF: &str = "off";
const UNARY: &str = "unary_expression";
const OPERATOR_FIELD: &str = "operator";
const TYPEOF: &str = "typeof";

// Los globales de todos los entornos que suele declarar un repo (ES, navegador, Node, CommonJS,
// workers y los frameworks de test): kerbo no lee la configuración de ESLint.
static GLOBALS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    KNOWN_GLOBALS
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.starts_with(HEADER_MARK))
        .collect()
});
// La lib de TypeScript que typescript-eslint declara en el alcance global: los tipos resuelven
// referencias de tipo y los valores, también las de valor.
static LIB: LazyLock<Vec<(&'static str, bool)>> = LazyLock::new(|| {
    TYPESCRIPT_LIB
        .lines()
        .filter(|line| !line.starts_with(HEADER_MARK))
        .filter_map(|line| line.split_once(LIB_SEPARATOR))
        .map(|(kind, name)| (name.trim(), kind == LIB_VALUE))
        .collect()
});
static AROUND_VALUE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\s*:\s*").ok());
static ENTRY_SEPARATOR: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"[\s,]+").ok());
static GLOBAL_COMMENT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\s*globals?(?:\s+(?<names>[\s\S]*?))?(?:\s-{2,}\s[\s\S]*)?$").ok());

// `no-undef`: un nombre que no se declara en ningún alcance, ni es un global conocido ni está en un
// comentario `/* global */`. Como ESLint, `typeof x` no cuenta.
pub fn undefined_names(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let declared_globals = comment_globals(context);
    let global_names = tree.scope(GLOBAL).map(|scope| &scope.names);
    let mut previous: Option<usize> = None;
    let mut found = Vec::new();
    for reference in tree
        .references
        .iter()
        .filter(|reference| reference.resolved.is_none())
    {
        let identifier = reference.identifier;
        let name = context.text(identifier);
        let in_lib = LIB
            .iter()
            .any(|(lib_name, is_value)| *lib_name == name && (*is_value || reference.is_type()));
        let known = GLOBALS.contains(name)
            || in_lib
            || declared_globals.contains(name)
            || global_names.is_some_and(|names| names.contains_key(name));
        if known || previous == Some(identifier.id()) || is_typeof_operand(identifier, context) {
            continue;
        }
        previous = Some(identifier.id());
        found.push(report(EslintRule::NoUndef, identifier, name));
    }
    found
}

fn is_typeof_operand(identifier: tree_sitter::Node<'_>, context: &Context<'_, '_>) -> bool {
    parent_without_parentheses(identifier).is_some_and(|parent| {
        parent.kind() == UNARY
            && parent
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| context.text(operator) == TYPEOF)
    })
}

// Los nombres de `/* global a, b: writable */` (no `: off`), sólo en comentarios de bloque, con el
// `parseStringConfig` de ESLint: separados por comas o espacios.
fn comment_globals(context: &Context<'_, '_>) -> HashSet<String> {
    let (Some(pattern), Some(around_value), Some(separator)) = (
        GLOBAL_COMMENT.as_ref(),
        AROUND_VALUE.as_ref(),
        ENTRY_SEPARATOR.as_ref(),
    ) else {
        return HashSet::new();
    };
    context
        .of_kind(&[COMMENT])
        .filter_map(|comment| {
            context
                .text(comment)
                .strip_prefix(BLOCK_PREFIX)?
                .strip_suffix(BLOCK_SUFFIX)
        })
        .filter_map(|value| pattern.captures(value)?.name("names"))
        .map(|names| {
            around_value
                .replace_all(names.as_str(), NORMALIZED_VALUE)
                .into_owned()
        })
        .flat_map(|names| {
            separator
                .split(&names)
                .map(str::to_owned)
                .collect::<Vec<String>>()
        })
        .filter_map(|entry| {
            let (name, value) = entry
                .split_once(VALUE_SEPARATOR)
                .map_or((entry.as_str(), ""), |(name, value)| (name, value.trim()));
            let name = name.trim();
            (!name.is_empty() && value != TURNED_OFF).then(|| name.to_owned())
        })
        .collect()
}
