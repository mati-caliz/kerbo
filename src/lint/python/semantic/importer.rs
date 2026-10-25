use tree_sitter::Node;

use super::model::{BindingKind, Model, Resolution};
use crate::fix::{TextEdit, TextPoint};
use crate::lint::python::ast::{
    COMMENT, EXPRESSION_STATEMENT, FUTURE_IMPORT, IMPORT, IMPORT_FROM, STRING, named, text,
};
use crate::lint::python::fix_edits::point_of;

const LINE_FEED: u8 = b'\n';
const MODULE_SEPARATOR: char = '.';

// El nombre con que se usa `módulo.miembro` en `at` y la edición que hace falta: un no-op sobre el
// import que ya está (para chocar con un F401 de la misma vuelta), o un `import módulo` nuevo. Es
// `get_or_import_symbol` de ruff con `ImportStyle::Import`; `None` si el nombre está tomado.
#[derive(Debug, Clone, Copy)]
pub struct ImportRequest<'request> {
    pub module: &'request str,
    pub member: &'request str,
}

pub fn get_or_import_member(
    model: &Model<'_>,
    source: &[u8],
    at: Node<'_>,
    request: ImportRequest<'_>,
) -> Option<(String, TextEdit)> {
    let ImportRequest { module, member } = request;
    if let Some(found) = existing_import(model, source, request) {
        return Some(found);
    }
    let available = !matches!(model.lookup_from(model.scope, module), Resolution::Binding(_));
    available.then(|| {
        let edit = add_import(source, at, &format!("import {module}"));
        (format!("{module}.{member}"), edit)
    })
}

// `resolve_qualified_import_name`: `import módulo` (con o sin alias) da `alias.miembro`, y
// `from módulo import miembro` da el nombre ligado, si no los tapa otro nombre.
fn existing_import(
    model: &Model<'_>,
    source: &[u8],
    request: ImportRequest<'_>,
) -> Option<(String, TextEdit)> {
    let ImportRequest { module, member } = request;
    let qualified_member = format!("{module}{MODULE_SEPARATOR}{member}");
    model.ancestors(model.scope).find_map(|scope| {
        model.scope_of(scope).names.iter().find_map(|(name, id)| {
            let binding = model.binding(*id)?;
            let reference = if binding.kind == BindingKind::Import && binding.qualified == module {
                format!("{name}{MODULE_SEPARATOR}{member}")
            } else if binding.kind == BindingKind::FromImport && binding.qualified == qualified_member {
                name.clone()
            } else {
                return None;
            };
            let visible = model.lookup_from(model.scope, name) == Resolution::Binding(*id);
            let statement = binding.site.statement?;
            visible.then(|| (reference, no_op(statement, source)))
        })
    })
}

fn no_op(statement: Node<'_>, source: &[u8]) -> TextEdit {
    TextEdit {
        start: TextPoint::from_point(statement.start_position()),
        end: TextPoint::from_point(statement.end_position()),
        replacement: text(statement, source).to_owned(),
    }
}

// `add_import`: después del último import de primer nivel antes de `at`; si no hay, después del
// último `from __future__`, o al principio del archivo pasando el docstring y los comentarios.
fn add_import(source: &[u8], at: Node<'_>, statement: &str) -> TextEdit {
    let root = std::iter::successors(Some(at), Node::parent).last().unwrap_or(at);
    let body: Vec<Node<'_>> = named(root)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    let imports: Vec<Node<'_>> = body
        .iter()
        .copied()
        .filter(|child| matches!(child.kind(), IMPORT | IMPORT_FROM | FUTURE_IMPORT))
        .collect();
    let preceding = imports
        .iter()
        .rev()
        .find(|import| import.start_byte() < at.start_byte())
        .or_else(|| imports.iter().rev().find(|import| import.kind() == FUTURE_IMPORT));
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
