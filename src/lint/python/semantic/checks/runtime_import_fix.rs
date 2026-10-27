use tree_sitter::Node;

use super::import_rebuild::rebuilt_import;
use super::removal::removal;
use crate::fix::Fix;
use crate::lint::python::semantic::importer::add_import;
use crate::lint::python::statement_removal::isolation_group;

// El `move_imports` de ruff: saca los nombres del `if TYPE_CHECKING:` y los importa en ejecución
// después del último import de primer nivel antes del primer uso (o al principio del archivo).
// Inseguro y aislado en el bloque, como en ruff.
pub fn move_out_of_type_checking(
    statement: Node<'_>,
    moved: &[Node<'_>],
    first_use: Node<'_>,
    source: &[u8],
) -> Option<Fix> {
    let root = std::iter::successors(Some(statement), Node::parent).last()?;
    let removed = removal(statement, moved, source)?;
    let content = rebuilt_import(statement, moved, source)?;
    let mut edits = removed.edits;
    edits.push(add_import(source, root, first_use, &content));
    Some(
        Fix::from_edits(source, edits)
            .marked_unsafe()
            .isolated(isolation_group(statement)),
    )
}
