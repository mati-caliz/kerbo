use tree_sitter::Node;

use super::model::{
    Alias, AliasComments, Annotated, AnnotatedAlias, Comments, FromModule, FromParts, SourceText,
};
use crate::lint::python::ast::{ALIAS_FIELD, ALIASED_IMPORT, COMMENT, IMPORT, NAME_FIELD, text};

const MODULE_NAME_FIELD: &str = "module_name";
const RELATIVE_IMPORT: &str = "relative_import";
const IMPORT_PREFIX: &str = "import_prefix";
const WILDCARD_IMPORT: &str = "wildcard_import";
const DOT: char = '.';
const STAR: &str = "*";
const OPENING: &str = "(";
const CLOSING: &str = ")";
const COMMA: &str = ",";
const FUTURE: &str = "__future__";
const FUTURE_IMPORT: &str = "future_import_statement";

// `annotate_imports` de ruff: cada import con los comentarios que le tocan, en el orden en que
// aparecen. `comments` va de la primera sentencia del bloque al final de la línea de la última.
pub fn annotate<'tree, 'source>(
    imports: &[Node<'tree>],
    comments: &[Node<'tree>],
    text_source: SourceText<'source>,
) -> Vec<Annotated<'source>> {
    let mut pending = Comments::new(comments, text_source);
    imports
        .iter()
        .map(|import| {
            if import.kind() == IMPORT {
                annotate_import(*import, &mut pending, text_source)
            } else {
                annotate_from(*import, &mut pending, text_source)
            }
        })
        .collect()
}

fn annotate_import<'source>(
    import: Node<'_>,
    pending: &mut Comments<'_, 'source>,
    text_source: SourceText<'source>,
) -> Annotated<'source> {
    let atop = pending.take_while_start_before(import.start_byte());
    let inline = pending.take_while_end_at_most(text_source.line_end(import.end_byte()));
    let names = named_children(import, NAME_FIELD)
        .into_iter()
        .map(|name| alias(name, text_source.bytes))
        .collect();
    Annotated::Import { names, atop, inline }
}

fn annotate_from<'source>(
    import: Node<'_>,
    pending: &mut Comments<'_, 'source>,
    text_source: SourceText<'source>,
) -> Annotated<'source> {
    let atop = pending.take_while_start_before(import.start_byte());
    let names = named_children(import, NAME_FIELD);
    let wildcard = named_children_of_kind(import, WILDCARD_IMPORT);
    let first_alias_start = names
        .first()
        .or_else(|| wildcard.first())
        .map_or_else(|| import.end_byte(), Node::start_byte);
    let breaks_line = text_source.has_line_break(import.start_byte(), first_alias_start);
    let inline = if names.len() > 1 || breaks_line {
        pending.take_while_end_at_most(text_source.line_end(import.start_byte()))
    } else {
        Vec::new()
    };
    let mut aliases: Vec<AnnotatedAlias<'source>> = names
        .iter()
        .chain(wildcard.iter())
        .map(|name| {
            let alias_atop = pending.take_while_start_before(name.start_byte());
            let alias_inline = pending.take_while_end_at_most(text_source.line_end(name.end_byte()));
            AnnotatedAlias {
                alias: if name.kind() == WILDCARD_IMPORT {
                    Alias {
                        name: STAR,
                        asname: None,
                    }
                } else {
                    alias(*name, text_source.bytes)
                },
                comments: AliasComments {
                    atop: alias_atop,
                    inline: alias_inline,
                    trailing: Vec::new(),
                },
            }
        })
        .collect();
    if let Some(last) = aliases.last_mut() {
        last.comments.trailing = pending.take_while_start_before(import.end_byte());
    }
    let trailing = pending.take_while_start_before(text_source.line_end(import.end_byte()));
    Annotated::From(FromParts {
        module: from_module(import, text_source.bytes),
        aliases,
        comments: AliasComments {
            atop,
            inline,
            trailing,
        },
        trailing_comma: has_trailing_comma(import, text_source.bytes),
    })
}

fn alias<'source>(name: Node<'_>, source: &'source [u8]) -> Alias<'source> {
    if name.kind() == ALIASED_IMPORT {
        Alias {
            name: name
                .child_by_field_name(NAME_FIELD)
                .map_or("", |inner| text(inner, source)),
            asname: name
                .child_by_field_name(ALIAS_FIELD)
                .map(|alias| text(alias, source)),
        }
    } else {
        Alias {
            name: text(name, source),
            asname: None,
        }
    }
}

fn from_module<'source>(import: Node<'_>, source: &'source [u8]) -> FromModule<'source> {
    if import.kind() == FUTURE_IMPORT {
        return FromModule {
            module: Some(FUTURE),
            level: 0,
        };
    }
    let Some(module) = import.child_by_field_name(MODULE_NAME_FIELD) else {
        return FromModule {
            module: None,
            level: 0,
        };
    };
    if module.kind() != RELATIVE_IMPORT {
        return FromModule {
            module: Some(text(module, source)),
            level: 0,
        };
    }
    let parts = crate::lint::python::ast::named(module);
    let level = parts
        .iter()
        .find(|part| part.kind() == IMPORT_PREFIX)
        .map_or(0, |prefix| {
            text(*prefix, source)
                .chars()
                .filter(|character| *character == DOT)
                .count()
        });
    let name = parts
        .iter()
        .find(|part| part.kind() != IMPORT_PREFIX)
        .map(|name| text(*name, source));
    FromModule { module: name, level }
}

// `trailing_comma`: el último token antes del `)` que cierra la lista es una coma.
fn has_trailing_comma(import: Node<'_>, source: &[u8]) -> bool {
    let mut cursor = import.walk();
    let tokens: Vec<Node<'_>> = import
        .children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect();
    let Some(opening) = tokens.iter().position(|token| text(*token, source) == OPENING) else {
        return false;
    };
    let inside = tokens.get(opening.saturating_add(1)..).unwrap_or_default();
    inside
        .iter()
        .take_while(|token| text(**token, source) != CLOSING)
        .last()
        .is_some_and(|token| text(*token, source) == COMMA)
}

fn named_children<'tree>(node: Node<'tree>, field: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

fn named_children_of_kind<'tree>(node: Node<'tree>, kind: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() == kind)
        .collect()
}
