use tree_sitter::{Node, Point};

use super::super::Visitor;
use super::super::model::{BindingId, MODULE_SCOPE, Reference};
use super::super::pep::is_typing_only;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, AUGMENTED_ASSIGNMENT, CALL, CONCATENATED_STRING, EXPRESSION_STATEMENT, FUNCTION_FIELD,
    INTERPOLATION, LEFT_FIELD, OBJECT_FIELD, STRING, named, text,
};
use crate::rules::PythonRule;

const DUNDER_ALL: &str = "__all__";
const MODULE_GETATTR: &str = "__getattr__";
const TYPE_PARAMETER: &str = "type_parameter";

// TC004: un import de adentro de `if TYPE_CHECKING:` que se usa en ejecución, donde el nombre no
// existe. Avisa en cada nombre importado, con el `noqa` también en la línea de la sentencia.
pub fn runtime_imports(visitor: &Visitor<'_>, latest: &[(&str, BindingId)]) -> Vec<Report> {
    latest
        .iter()
        .filter_map(|(_, id)| visitor.model.binding(*id))
        .filter(|binding| binding.kind.is_import())
        .filter_map(|binding| {
            let statement = binding.site.statement?;
            if !is_typing_only(statement, false, visitor.source) {
                return None;
            }
            let context = Context {
                visitor,
                root: std::iter::successors(Some(statement), Node::parent).last()?,
                deferred: visitor.boundaries.future_annotations(),
                module_getattr: visitor
                    .model
                    .scope_of(MODULE_SCOPE)
                    .names
                    .contains_key(MODULE_GETATTR),
            };
            let used_at_runtime = binding
                .references
                .iter()
                .any(|reference| context.is_runtime_reference(reference));
            used_at_runtime.then(|| {
                Report::at_point(PythonRule::TC004, binding.site.at, &binding.qualified)
                    .with_statement_row(statement.start_position().row)
            })
        })
        .collect()
}

struct Context<'visitor, 'tree> {
    visitor: &'visitor Visitor<'tree>,
    root: Node<'tree>,
    deferred: bool,
    module_getattr: bool,
}

impl Context<'_, '_> {
    // `in_runtime_context` de una referencia: fuera de `if TYPE_CHECKING:`, de una anotación que no se
    // evalúa, de una anotación entre comillas y de la cota de un parámetro de tipo. Un nombre en
    // `__all__` cuenta como uso salvo que el módulo tenga `__getattr__`.
    fn is_runtime_reference(&self, reference: &Reference) -> bool {
        let source = self.visitor.source;
        let Some(node) = node_at(self.root, reference.at) else {
            return false;
        };
        let string = std::iter::successors(Some(node), Node::parent)
            .find(|ancestor| matches!(ancestor.kind(), STRING | CONCATENATED_STRING | INTERPOLATION))
            .filter(|ancestor| ancestor.kind() != INTERPOLATION);
        if let Some(string) = string {
            return !self.module_getattr && is_dunder_all_entry(string, source);
        }
        let in_type_parameter =
            std::iter::successors(Some(node), Node::parent).any(|ancestor| ancestor.kind() == TYPE_PARAMETER);
        !in_type_parameter && !is_typing_only(node, self.deferred, source)
    }
}

pub fn node_at(root: Node<'_>, at: Point) -> Option<Node<'_>> {
    let inside = Point {
        row: at.row,
        column: at.column.saturating_add(1),
    };
    root.descendant_for_point_range(at, inside)
}

// Un string de `__all__ = [...]`, `__all__ += [...]` o `__all__.extend([...])`.
fn is_dunder_all_entry(string: Node<'_>, source: &[u8]) -> bool {
    let Some(statement) = std::iter::successors(Some(string), Node::parent)
        .find(|ancestor| ancestor.kind() == EXPRESSION_STATEMENT)
    else {
        return false;
    };
    named(statement).into_iter().any(|child| {
        let target = match child.kind() {
            ASSIGNMENT | AUGMENTED_ASSIGNMENT => child.child_by_field_name(LEFT_FIELD),
            CALL => child
                .child_by_field_name(FUNCTION_FIELD)
                .and_then(|function| function.child_by_field_name(OBJECT_FIELD)),
            _ => None,
        };
        target.is_some_and(|target| text(target, source) == DUNDER_ALL)
    })
}
