use tree_sitter::Node;

use super::type_hints::{Hints, unwrapped};
use crate::lint::python::ast::{COMMENT, is_string, named};
use crate::lint::python::escapes::text_value;
use crate::syntax::{SnippetLanguage, parse_snippet};

const PARAMETERS_FIELD: &str = "parameters";
const TYPE_FIELD: &str = "type";
const POSITIONAL_SEPARATOR: &str = "positional_separator";
const KEYWORD_SEPARATOR: &str = "keyword_separator";
const LIST_SPLAT_PATTERN: &str = "list_splat_pattern";
const DICTIONARY_SPLAT_PATTERN: &str = "dictionary_splat_pattern";
const DEFAULTED: [&str; 2] = ["default_parameter", "typed_default_parameter"];

// Los parámetros de una función como los separa ruff: `posonlyargs` y `args` juntos, `*args`, los
// que van después del `*` y `**kwargs`.
#[derive(Debug, Default)]
pub struct Signature<'tree> {
    pub node: Option<Node<'tree>>,
    pub positional: Vec<Node<'tree>>,
    pub variadic: Option<Node<'tree>>,
    pub keyword_only: Vec<Node<'tree>>,
    pub keywords: Option<Node<'tree>>,
}

impl<'tree> Signature<'tree> {
    pub fn of(function: Node<'tree>) -> Self {
        let node = function.child_by_field_name(PARAMETERS_FIELD);
        let mut signature = Self {
            node,
            ..Self::default()
        };
        let mut after_star = false;
        for parameter in node.map(named).unwrap_or_default() {
            if matches!(parameter.kind(), COMMENT | POSITIONAL_SEPARATOR) {
                continue;
            }
            if parameter.kind() == KEYWORD_SEPARATOR {
                after_star = true;
                continue;
            }
            match splat_kind(parameter) {
                Some(LIST_SPLAT_PATTERN) => {
                    signature.variadic = Some(parameter);
                    after_star = true;
                }
                Some(_) => signature.keywords = Some(parameter),
                None if after_star => signature.keyword_only.push(parameter),
                None => signature.positional.push(parameter),
            }
        }
        signature
    }

    pub fn count(&self) -> usize {
        self.positional
            .len()
            .saturating_add(self.keyword_only.len())
            .saturating_add(usize::from(self.variadic.is_some()))
            .saturating_add(usize::from(self.keywords.is_some()))
    }
}

fn splat_kind(parameter: Node<'_>) -> Option<&'static str> {
    std::iter::once(parameter)
        .chain(named(parameter))
        .find_map(|node| match node.kind() {
            LIST_SPLAT_PATTERN => Some(LIST_SPLAT_PATTERN),
            DICTIONARY_SPLAT_PATTERN => Some(DICTIONARY_SPLAT_PATTERN),
            _ => None,
        })
}

pub fn annotation(parameter: Node<'_>) -> Option<Node<'_>> {
    parameter.child_by_field_name(TYPE_FIELD)
}

pub fn has_default(parameter: Node<'_>) -> bool {
    DEFAULTED.contains(&parameter.kind())
}

// `match_maybe_stringized_annotation` de ruff: la anotación, o lo que hay adentro si es un string.
pub fn matches_annotation(
    hints: Hints<'_, '_, '_>,
    annotation: Node<'_>,
    predicate: &dyn Fn(Hints<'_, '_, '_>, Node<'_>) -> bool,
) -> bool {
    let annotation = unwrapped(annotation);
    if !is_string(annotation) {
        return predicate(hints, annotation);
    }
    let Some(value) = text_value(annotation, hints.source) else {
        return false;
    };
    let value = value.trim();
    let Some(tree) = parse_snippet(SnippetLanguage::Python, value) else {
        return false;
    };
    let root = tree.root_node();
    let expression = named(root)
        .into_iter()
        .next()
        .and_then(|statement| named(statement).into_iter().next());
    let inner = Hints {
        model: hints.model,
        source: value.as_bytes(),
    };
    !root.has_error() && expression.is_some_and(|expression| predicate(inner, unwrapped(expression)))
}
