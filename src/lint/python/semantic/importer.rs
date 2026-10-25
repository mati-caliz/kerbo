use tree_sitter::Node;

use super::model::{BindingKind, Model, Resolution};
use super::pep::is_typing_only;
use crate::fix::{TextEdit, TextPoint};
use crate::lint::python::ast::{
    ALIASED_IMPORT, COMMENT, EXPRESSION_STATEMENT, FUTURE_IMPORT, IMPORT, IMPORT_FROM, MODULE_NAME_FIELD,
    NAME_FIELD, STRING, WILDCARD_IMPORT, fields, named, text,
};
use crate::lint::python::fix_edits::point_of;

const LINE_FEED: u8 = b'\n';
const MODULE_SEPARATOR: char = '.';
const COMMA: &str = ",";
const CLOSING: &str = ")";
const FUTURE_ANNOTATIONS: &str = "annotations";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStyle {
    Import,
    ImportFrom,
}

#[derive(Debug, Clone, Copy)]
pub struct ImportRequest<'request> {
    pub module: &'request str,
    pub member: &'request str,
    pub style: ImportStyle,
}

struct Imported<'tree> {
    reference: String,
    statement: Node<'tree>,
}

// El `get_or_import_symbol` de ruff: el nombre con que se usa `módulo.miembro` en `at` y la edición
// que hace falta. Si ya está importado, un no-op sobre ese import (para chocar con un F401 de la
// misma vuelta); si no, el miembro sumado a un `from módulo import ...` que ya esté, o un import
// nuevo. `None` donde ruff no arregla: el import está después del uso, es sólo de tipado y el uso es
// en ejecución, o el nombre está tomado.
pub fn get_or_import_member(
    model: &Model<'_>,
    source: &[u8],
    at: Node<'_>,
    request: ImportRequest<'_>,
) -> Option<(String, TextEdit)> {
    if let Some(imported) = existing_import(model, request) {
        if imported.statement.start_byte() > at.start_byte() {
            return None;
        }
        let future = has_future_annotations(at, source);
        if is_typing_only(imported.statement, false, source) && !is_typing_only(at, future, source) {
            return None;
        }
        return Some((imported.reference, no_op(imported.statement, source)));
    }
    let ImportRequest {
        module,
        member,
        style,
    } = request;
    let available = |name: &str| !matches!(model.lookup_from(model.scope, name), Resolution::Binding(_));
    let root = std::iter::successors(Some(at), Node::parent).last().unwrap_or(at);
    if let Some(statement) = import_from_statement(root, module, at, source) {
        return available(member).then(|| (member.to_owned(), add_member(statement, member, source)));
    }
    match style {
        ImportStyle::Import => available(module).then(|| {
            let edit = add_import(source, root, at, &format!("import {module}"));
            (format!("{module}{MODULE_SEPARATOR}{member}"), edit)
        }),
        ImportStyle::ImportFrom => available(member).then(|| {
            let edit = add_import(source, root, at, &format!("from {module} import {member}"));
            (member.to_owned(), edit)
        }),
    }
}

// `resolve_qualified_import_name`: `import módulo` da `alias.miembro`, `import módulo.sub` da
// `módulo.miembro` y `from módulo import miembro` da el nombre ligado, si no los tapa un nombre de
// un alcance más adentro. Entre varios del mismo alcance, el último.
fn existing_import<'tree>(model: &Model<'tree>, request: ImportRequest<'_>) -> Option<Imported<'tree>> {
    let ImportRequest { module, member, .. } = request;
    let qualified_member = format!("{module}{MODULE_SEPARATOR}{member}");
    let submodule_prefix = format!("{module}{MODULE_SEPARATOR}");
    model.ancestors(model.scope).find_map(|scope| {
        let mut found: Vec<Imported<'tree>> = model
            .scope_of(scope)
            .names
            .iter()
            .filter_map(|(name, id)| {
                let binding = model.binding(*id)?;
                let is_submodule = binding.kind == BindingKind::SubmoduleImport
                    && (binding.qualified == module || binding.qualified.starts_with(&submodule_prefix));
                let reference = if binding.kind == BindingKind::Import && binding.qualified == module {
                    format!("{name}{MODULE_SEPARATOR}{member}")
                } else if binding.kind == BindingKind::FromImport && binding.qualified == qualified_member {
                    name.clone()
                } else if is_submodule {
                    qualified_member.clone()
                } else {
                    return None;
                };
                let visible = model.lookup_from(model.scope, name) == Resolution::Binding(*id);
                let statement = binding.site.statement?;
                visible.then_some(Imported { reference, statement })
            })
            .collect();
        found.sort_by_key(|imported| imported.statement.start_byte());
        found.pop()
    })
}

fn no_op(statement: Node<'_>, source: &[u8]) -> TextEdit {
    TextEdit {
        start: TextPoint::from_point(statement.start_position()),
        end: TextPoint::from_point(statement.end_position()),
        replacement: text(statement, source).to_owned(),
    }
}

fn top_level_imports(root: Node<'_>) -> Vec<Node<'_>> {
    named(root)
        .into_iter()
        .filter(|child| matches!(child.kind(), IMPORT | IMPORT_FROM | FUTURE_IMPORT))
        .collect()
}

// `find_import_from`: el último `from módulo import ...` de primer nivel antes de `at`, sin `*`.
fn import_from_statement<'tree>(
    root: Node<'tree>,
    module: &str,
    at: Node<'_>,
    source: &[u8],
) -> Option<Node<'tree>> {
    top_level_imports(root)
        .into_iter()
        .take_while(|statement| statement.start_byte() < at.start_byte())
        .filter(|statement| {
            statement.kind() == IMPORT_FROM
                && statement
                    .child_by_field_name(MODULE_NAME_FIELD)
                    .is_some_and(|name| text(name, source) == module)
                && !named(*statement)
                    .iter()
                    .any(|child| child.kind() == WILDCARD_IMPORT)
        })
        .last()
}

// `add_member` de ruff con libcst: el miembro nuevo va al final, con la misma coma que el último.
fn add_member(statement: Node<'_>, member: &str, source: &[u8]) -> TextEdit {
    let names = fields(statement, NAME_FIELD);
    let last = names.last().copied().unwrap_or(statement);
    let mut cursor = statement.walk();
    let after: Vec<Node<'_>> = statement
        .children(&mut cursor)
        .filter(|child| child.start_byte() >= last.end_byte())
        .collect();
    let trailing = after
        .first()
        .filter(|comma| comma.kind() == COMMA)
        .zip(after.get(1).filter(|closing| closing.kind() == CLOSING));
    let (start, end, replacement) = match trailing {
        Some((comma, closing)) => {
            let whitespace = String::from_utf8_lossy(
                source
                    .get(comma.end_byte()..closing.start_byte())
                    .unwrap_or_default(),
            )
            .into_owned();
            (
                comma.end_byte(),
                closing.start_byte(),
                format!("{whitespace}{member},{whitespace}"),
            )
        }
        None => (last.end_byte(), last.end_byte(), format!(", {member}")),
    };
    TextEdit {
        start: point_of(source, start),
        end: point_of(source, end),
        replacement,
    }
}

// `add_import`: después del último import de primer nivel antes de `at`; si no hay, después del
// último `from __future__`, o al principio del archivo pasando el docstring y los comentarios.
fn add_import(source: &[u8], root: Node<'_>, at: Node<'_>, statement: &str) -> TextEdit {
    let imports = top_level_imports(root);
    let preceding = imports
        .iter()
        .rev()
        .find(|import| import.start_byte() < at.start_byte())
        .or_else(|| imports.iter().rev().find(|import| import.kind() == FUTURE_IMPORT));
    let body: Vec<Node<'_>> = named(root)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    let offset = preceding.map_or_else(
        || start_of_file(&body, source),
        |import| line_end(source, import.end_byte()),
    );
    let point = point_of(source, offset);
    TextEdit {
        start: point,
        end: point,
        replacement: format!("{statement}\n"),
    }
}

fn has_future_annotations(at: Node<'_>, source: &[u8]) -> bool {
    let root = std::iter::successors(Some(at), Node::parent).last().unwrap_or(at);
    top_level_imports(root)
        .into_iter()
        .filter(|statement| statement.kind() == FUTURE_IMPORT)
        .flat_map(|statement| fields(statement, NAME_FIELD))
        .any(|name| {
            let name = if name.kind() == ALIASED_IMPORT {
                name.child_by_field_name(NAME_FIELD).unwrap_or(name)
            } else {
                name
            };
            text(name, source) == FUTURE_ANNOTATIONS
        })
}

fn line_end(source: &[u8], offset: usize) -> usize {
    source
        .get(offset..)
        .and_then(|rest| rest.iter().position(|byte| *byte == LINE_FEED))
        .map_or(source.len(), |newline| {
            offset.saturating_add(newline).saturating_add(1)
        })
}

// `Insertion::start_of_file`: después del docstring y de los comentarios que siguen al principio.
fn start_of_file(body: &[Node<'_>], source: &[u8]) -> usize {
    let docstring = body.first().filter(|first| {
        first.kind() == EXPRESSION_STATEMENT
            && named(**first).first().is_some_and(|value| value.kind() == STRING)
    });
    let mut location = docstring.map_or(0, |docstring| line_end(source, docstring.end_byte()));
    let mut cursor = location;
    let mut seen_comment = false;
    while cursor < source.len() {
        let end = line_end(source, cursor);
        let line = String::from_utf8_lossy(source.get(cursor..end).unwrap_or_default());
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if seen_comment {
                location = end;
            }
        } else if trimmed.starts_with('#') {
            location = end;
            seen_comment = true;
        } else {
            break;
        }
        cursor = end;
    }
    location
}
