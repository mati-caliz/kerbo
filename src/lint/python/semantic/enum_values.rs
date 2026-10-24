use tree_sitter::Node;

use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, BODY_FIELD, CALL, COMMENT, EXPRESSION_STATEMENT, FUNCTION_FIELD, RIGHT_FIELD, TYPE_FIELD,
    named,
};
use crate::lint::python::branches::equivalent;
use crate::rules::PythonRule;

const SUPERCLASSES_FIELD: &str = "superclasses";
const ENUM: &str = "enum.Enum";
const AUTO: &str = "enum.auto";

// PIE796: dos miembros de una clase que hereda de `enum.Enum` con el mismo valor; el segundo pasa a
// ser un alias del primero. Avisa en la asignación repetida.
pub fn duplicate_enum_values(model: &Model<'_>, source: &[u8], class: Node<'_>) -> Vec<Report> {
    let qualifies =
        |node: Node<'_>, wanted: &str| qualified_name(model, node, source).as_deref() == Some(wanted);
    let is_enum = class
        .child_by_field_name(SUPERCLASSES_FIELD)
        .is_some_and(|bases| named(bases).into_iter().any(|base| qualifies(base, ENUM)));
    let Some(body) = class.child_by_field_name(BODY_FIELD).filter(|_| is_enum) else {
        return Vec::new();
    };
    let mut seen: Vec<Node<'_>> = Vec::new();
    let mut found = Vec::new();
    for assignment in named(body)
        .into_iter()
        .filter(|statement| statement.kind() == EXPRESSION_STATEMENT)
        .filter_map(|statement| named(statement).into_iter().find(|child| child.kind() != COMMENT))
        .filter(|assignment| {
            assignment.kind() == ASSIGNMENT && assignment.child_by_field_name(TYPE_FIELD).is_none()
        })
    {
        let Some(value) = assignment.child_by_field_name(RIGHT_FIELD) else {
            continue;
        };
        let is_auto = value.kind() == CALL
            && value
                .child_by_field_name(FUNCTION_FIELD)
                .is_some_and(|function| qualifies(function, AUTO));
        if is_auto || value.kind() == ASSIGNMENT {
            continue;
        }
        if seen.iter().any(|earlier| equivalent(*earlier, value, source)) {
            found.push(Report::at(PythonRule::PIE796, assignment, ""));
        } else {
            seen.push(value);
        }
    }
    found
}
