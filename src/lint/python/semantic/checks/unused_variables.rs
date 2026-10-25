use tree_sitter::Node;

use super::super::effects::contains_effect;
use super::super::model::{Binding, BindingKind, Model};
use super::super::qualified::qualified_name_from;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{ASSIGNMENT, LEFT_FIELD, RIGHT_FIELD, TYPE_FIELD, text, unparenthesized};
use crate::lint::python::statement_removal::{delete_statement, isolation_group};

const BUILTINS_PREFIX: &str = "builtins.";
const AS_PATTERN: &str = "as_pattern";
const AS_PATTERN_TARGET: &str = "as_pattern_target";
const DUMMY_PREFIX: char = '_';

// El arreglo de F841 de ruff, siempre inseguro: borrar la asignación de un valor sin efectos, dejar
// sólo el valor si tiene efectos, renombrar a `_x` lo desempaquetado y sacar el `as x` de un `with`.
pub fn unused_variable_fix(model: &Model<'_>, source: &[u8], binding: &Binding<'_>) -> Option<Fix> {
    let fix = if binding.kind == BindingKind::Assignment {
        assignment_fix(model, source, binding)
    } else if binding.kind == BindingKind::WithItem {
        with_item_fix(binding.site.node?, source)
    } else {
        None
    };
    fix.map(Fix::marked_unsafe)
}

// RUF059: `a, b = f()` con `b` sin usar pasa a `a, _b = f()`, inseguro como en ruff.
pub fn unpacked_variable_fix(source: &[u8], binding: &Binding<'_>) -> Option<Fix> {
    let name = binding.site.node?;
    let statement = binding.site.statement?;
    dummy_rename(source, name, statement).map(Fix::marked_unsafe)
}

fn assignment_fix(model: &Model<'_>, source: &[u8], binding: &Binding<'_>) -> Option<Fix> {
    let name = binding.site.node?;
    let statement = binding.site.statement?;
    let top = statement
        .named_child(0)
        .filter(|child| child.kind() == ASSIGNMENT)?;
    let chain = assignment_chain(top);
    let value = chain.last()?.child_by_field_name(RIGHT_FIELD)?;
    let owner = chain.iter().copied().find(|assignment| {
        assignment
            .child_by_field_name(LEFT_FIELD)
            .is_some_and(|target| unparenthesized(target).id() == name.id())
    });
    let Some(owner) = owner else {
        return dummy_rename(source, name, statement);
    };
    let is_builtin = |node: Node<'_>| {
        qualified_name_from(model, binding.scope, node, source)
            .is_some_and(|qualified| qualified.strip_prefix(BUILTINS_PREFIX) == Some(text(node, source)))
    };
    let annotated = owner.child_by_field_name(TYPE_FIELD).is_some();
    if (!annotated && chain.len() > 1) || contains_effect(value, source, &is_builtin) {
        let start = if annotated {
            owner
        } else {
            owner.child_by_field_name(LEFT_FIELD)?
        };
        let end = owner.child_by_field_name(RIGHT_FIELD)?;
        return Some(deletion(start, end, source));
    }
    Some(delete_statement(statement, source))
}

// `a = b = valor`: cada asignación de la cadena, de afuera hacia adentro; con anotación no hay
// cadena.
fn assignment_chain(top: Node<'_>) -> Vec<Node<'_>> {
    let mut chain = vec![top];
    while let Some(inner) = chain
        .last()
        .filter(|assignment| assignment.child_by_field_name(TYPE_FIELD).is_none())
        .and_then(|assignment| assignment.child_by_field_name(RIGHT_FIELD))
        .filter(|right| right.kind() == ASSIGNMENT)
    {
        chain.push(inner);
    }
    chain
}

// Un nombre adentro de `a, b = ...`: ruff lo renombra a `_b` si el nombre nuevo pasa por variable de
// relleno, o sea, si termina en letra o número.
fn dummy_rename(source: &[u8], name: Node<'_>, statement: Node<'_>) -> Option<Fix> {
    let current = text(name, source);
    let renamed = format!("{DUMMY_PREFIX}{current}");
    let is_dummy = current.chars().last().is_some_and(char::is_alphanumeric);
    is_dummy.then(|| Fix::replace(name, &renamed, source).isolated(isolation_group(statement)))
}

// `with abrir() as x:` pasa a `with abrir():`.
fn with_item_fix(name: Node<'_>, source: &[u8]) -> Option<Fix> {
    let target = name
        .parent()
        .filter(|parent| parent.kind() == AS_PATTERN_TARGET)?;
    let pattern = target.parent().filter(|parent| parent.kind() == AS_PATTERN)?;
    let value = pattern.named_child(0)?;
    let edit = TextEdit {
        start: TextPoint::from_point(value.end_position()),
        end: TextPoint::from_point(target.end_position()),
        replacement: String::new(),
    };
    Some(Fix::from_edits(source, vec![edit]))
}

fn deletion(start: Node<'_>, end: Node<'_>, source: &[u8]) -> Fix {
    let edit = TextEdit {
        start: TextPoint::from_point(start.start_position()),
        end: TextPoint::from_point(end.start_position()),
        replacement: String::new(),
    };
    Fix::from_edits(source, vec![edit])
}
