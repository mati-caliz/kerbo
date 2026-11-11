use tree_sitter::Node;

use crate::lint::java::children;
use crate::syntax::named_children;

const PACKAGE: &str = "package_declaration";
const ANNOTATION: &str = "marker_annotation";
const ANNOTATIONS: &[&str] = &["marker_annotation", "annotation"];
const IMPORT: &str = "import_declaration";
const STATIC: &str = "static";
const ASTERISK: &str = "asterisk";
const WILDCARD_SUFFIX: &str = ".*";
const NAME_FIELD: &str = "name";
const QUALIFIER_SEPARATOR: char = '.';

pub(super) fn package_of(root: Node<'_>, source: &[u8]) -> String {
    package_declaration(root)
        .and_then(|package| {
            named_children(package)
                .into_iter()
                .find(|child| child.kind() != ANNOTATION)
        })
        .and_then(|name| name.utf8_text(source).ok())
        .unwrap_or_default()
        .to_owned()
}

pub(super) fn package_annotations(root: Node<'_>, source: &[u8]) -> Vec<String> {
    package_declaration(root)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|child| ANNOTATIONS.contains(&child.kind()))
        .filter_map(|annotation| annotation.child_by_field_name(NAME_FIELD))
        .filter_map(|name| name.utf8_text(source).ok())
        .map(simple_name)
        .collect()
}

pub(super) fn imported_paths(root: Node<'_>, source: &[u8], static_imports: bool) -> Vec<String> {
    named_children(root)
        .into_iter()
        .filter(|node| node.kind() == IMPORT)
        .filter(|import| children(*import).iter().any(|child| child.kind() == STATIC) == static_imports)
        .filter_map(|import| imported_path(import, source))
        .collect()
}

fn package_declaration(root: Node<'_>) -> Option<Node<'_>> {
    named_children(root)
        .into_iter()
        .find(|node| node.kind() == PACKAGE)
}

// `import a.b.C;` da `a.b.C` y `import a.b.*;` da `a.b.*`.
fn imported_path(import: Node<'_>, source: &[u8]) -> Option<String> {
    let parts = named_children(import);
    let path = parts.first()?.utf8_text(source).ok()?;
    let wildcard = parts.iter().any(|part| part.kind() == ASTERISK);
    Some(if wildcard {
        format!("{path}{WILDCARD_SUFFIX}")
    } else {
        path.to_owned()
    })
}

fn simple_name(written: &str) -> String {
    written
        .rsplit(QUALIFIER_SEPARATOR)
        .next()
        .unwrap_or(written)
        .to_owned()
}
