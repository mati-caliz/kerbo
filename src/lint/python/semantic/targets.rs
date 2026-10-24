use tree_sitter::Node;

use super::aliases::handler_alias_reports;
use super::handlers::{CONTINUE, PASS, blind_except, is_silenced_exception};
use super::model::{BindingKind, Resolution, ScopeKind, Site, UNPACKED};
use super::{Mode, Visitor, is_dummy};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, ATTRIBUTE, CLASS, FUNCTION, IDENTIFIER, LEFT_FIELD, LIST, OBJECT_FIELD, PARENTHESIZED,
    RIGHT_FIELD, SET, SUBSCRIPT, TUPLE, TYPE_FIELD, VALUE_FIELD, is_string, named, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::semantic::annotations::Annotations;
use crate::lint::python::semantic::checks::Exports;
use crate::lint::python::semantic::expressions::Expressions;
use crate::lint::python::semantic::statements::Statements;
use crate::lint::python::typing_names::is_snake_case;
use crate::rules::PythonRule;

const DUNDER_ALL: &str = "__all__";
const SEQUENCE_TARGETS: [&str; 5] = ["pattern_list", "tuple_pattern", "list_pattern", TUPLE, LIST];
const EXPRESSION_LIST: &str = "expression_list";
const SEQUENCE_VALUES: [&str; 4] = [TUPLE, LIST, SET, "expression_list"];
const NESTED_TARGETS: [&str; 8] = [
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    TUPLE,
    LIST,
    PARENTHESIZED,
    "list_splat_pattern",
    "list_splat",
];
const DELETED_SEQUENCES: [&str; 4] = [TUPLE, LIST, "expression_list", PARENTHESIZED];
const EXCEPT: &str = "except_clause";
const CONDITIONAL_PARENTS: [&str; 3] = ["if_statement", "while_statement", "match_statement"];
const BINARY: &str = "binary_operator";
const PLUS: &str = "+";
const OPERATOR_FIELD: &str = "operator";

#[derive(Debug, Clone, Copy)]
pub struct Store<'tree> {
    pub kind: BindingKind,
    pub statement: Node<'tree>,
    pub flags: u8,
}

impl<'tree> Store<'tree> {
    pub const fn plain(kind: BindingKind, statement: Node<'tree>) -> Self {
        Self {
            kind,
            statement,
            flags: 0,
        }
    }

    pub const fn unpacked(mut self, unpacked: bool) -> Self {
        if unpacked {
            self.flags |= UNPACKED;
        }
        self
    }
}

pub trait Targets<'tree> {
    fn assignment(&mut self, assignment: Node<'tree>, statement: Node<'tree>);
    fn augmented_assignment(&mut self, assignment: Node<'tree>, statement: Node<'tree>);
    fn is_export(&self, first: Option<Node<'tree>>) -> bool;
    fn export(&mut self, target: Node<'tree>, value: Option<Node<'tree>>, statement: Node<'tree>);
    fn export_names(&self, value: Node<'tree>, names: &mut Vec<(String, tree_sitter::Point)>);
    fn store(&mut self, target: Node<'tree>, store: Store<'tree>);
    fn bind_name(&mut self, name: Node<'tree>, store: Store<'tree>) -> usize;
    fn delete_target(&mut self, target: Node<'tree>, statement: Node<'tree>);
    fn delete_name(&mut self, name: Node<'tree>, statement: Node<'tree>);
    fn handler(&mut self, handler: Node<'tree>);
    fn bind_outside_branch(&mut self, name: Node<'tree>, store: Store<'tree>) -> usize;
    fn redefines_argument(&self, id: usize) -> bool;
}

impl<'tree> Targets<'tree> for Visitor<'tree> {
    fn assignment(&mut self, assignment: Node<'tree>, statement: Node<'tree>) {
        let mut targets = Vec::new();
        let mut current = assignment;
        let value = loop {
            targets.extend(current.child_by_field_name(LEFT_FIELD));
            match current.child_by_field_name(RIGHT_FIELD) {
                Some(next) if next.kind() == ASSIGNMENT => current = next,
                other => break other,
            }
        };
        let annotation = assignment.child_by_field_name(TYPE_FIELD);
        if let Some(annotation) = annotation {
            self.annotation(annotation);
        }
        if let Some(value) = value {
            let alias = annotation.is_some_and(|annotation| self.is_type_alias(annotation));
            self.expression(value, if alias { Mode::TypeDefinition } else { Mode::Value });
            if let Some(target) = targets
                .first()
                .filter(|target| alias && target.kind() == IDENTIFIER)
                && is_snake_case(self.text(*target))
            {
                self.reports.push(Report::at(PythonRule::PYI042, *target, ""));
            }
        }
        let exported = self.is_export(targets.first().copied());
        let kind = if annotation.is_some() && value.is_none() {
            BindingKind::Annotation
        } else {
            BindingKind::Assignment
        };
        let value_is_sequence = value.is_some_and(|value| SEQUENCE_VALUES.contains(&value.kind()));
        let all_sequences = targets
            .iter()
            .all(|target| SEQUENCE_TARGETS.contains(&target.kind()));
        for target in targets {
            if exported && target.kind() == IDENTIFIER {
                self.export(target, value, statement);
                continue;
            }
            let in_sequence = all_sequences || SEQUENCE_TARGETS.contains(&target.kind());
            let unpacked = in_sequence && (!value_is_sequence || !all_sequences);
            self.store(target, Store::plain(kind, statement).unpacked(unpacked));
        }
    }

    fn augmented_assignment(&mut self, assignment: Node<'tree>, statement: Node<'tree>) {
        let Some(target) = assignment.child_by_field_name(LEFT_FIELD) else {
            return;
        };
        if target.kind() == IDENTIFIER {
            self.model
                .resolve_load(self.text(target), target.start_position());
        }
        let value = assignment.child_by_field_name(RIGHT_FIELD);
        if let Some(value) = value {
            self.expression(value, Mode::Value);
        }
        if target.kind() == IDENTIFIER && self.is_export(Some(target)) {
            self.export(target, value, statement);
        } else {
            self.store(target, Store::plain(BindingKind::Assignment, statement));
        }
    }

    fn is_export(&self, first: Option<Node<'tree>>) -> bool {
        self.model.current().kind == ScopeKind::Module
            && first.is_some_and(|target| target.kind() == IDENTIFIER && self.text(target) == DUNDER_ALL)
    }

    fn export(&mut self, target: Node<'tree>, value: Option<Node<'tree>>, statement: Node<'tree>) {
        let mut names = Vec::new();
        if let Some(value) = value {
            self.export_names(value, &mut names);
        }
        self.exports.extend(names);
        self.bind_name(target, Store::plain(BindingKind::Export, statement));
    }

    fn export_names(&self, value: Node<'tree>, names: &mut Vec<(String, tree_sitter::Point)>) {
        let value = unparenthesized(value);
        if matches!(value.kind(), LIST | TUPLE | EXPRESSION_LIST) {
            names.extend(
                named(value)
                    .into_iter()
                    .filter(|element| is_string(*element))
                    .filter_map(|element| {
                        text_value(element, self.source).map(|name| (name, element.start_position()))
                    }),
            );
            return;
        }
        let is_plus = value.kind() == BINARY
            && value
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| self.text(operator) == PLUS);
        if !is_plus {
            return;
        }
        for side in [LEFT_FIELD, RIGHT_FIELD]
            .into_iter()
            .filter_map(|side| value.child_by_field_name(side))
        {
            self.export_names(side, names);
        }
    }

    fn store(&mut self, target: Node<'tree>, store: Store<'tree>) {
        match target.kind() {
            IDENTIFIER => {
                self.bind_name(target, store);
            }
            ATTRIBUTE => {
                if let Some(object) = target.child_by_field_name(OBJECT_FIELD) {
                    self.expression(object, Mode::Value);
                }
            }
            SUBSCRIPT => self.loads(target),
            kind if NESTED_TARGETS.contains(&kind) => {
                for inner in named(target) {
                    self.store(inner, store);
                }
            }
            _ => self.expression(target, Mode::Value),
        }
    }

    fn bind_name(&mut self, name: Node<'tree>, store: Store<'tree>) -> usize {
        let site = Site {
            at: name.start_position(),
            node: Some(name),
            statement: Some(store.statement),
        };
        let id = self.model.add_binding(self.text(name), store.kind, site);
        self.model.flag(id, store.flags);
        if self.redefines_argument(id) {
            self.reports.push(Report::at(PythonRule::PLR1704, name, ""));
        }
        id
    }

    // PLR1704: un `for`, un `with ... as` o un `except ... as` que pisa un parámetro de la función o
    // de una que la contiene.
    fn redefines_argument(&self, id: usize) -> bool {
        let Some(binding) = self.model.binding(id) else {
            return false;
        };
        let redefining = matches!(
            binding.kind,
            BindingKind::LoopVariable | BindingKind::WithItem | BindingKind::BoundException
        );
        redefining
            && binding.site.node.is_some_and(|name| !is_dummy(self.text(name)))
            && self
                .model
                .shadowed_in_scope(binding.scope, id)
                .or_else(|| self.model.cross_shadowed.get(&id).copied())
                .and_then(|shadowed| self.model.binding(shadowed))
                .is_some_and(|shadowed| shadowed.kind == BindingKind::Argument)
    }

    fn delete_target(&mut self, target: Node<'tree>, statement: Node<'tree>) {
        match target.kind() {
            IDENTIFIER => self.delete_name(target, statement),
            kind if DELETED_SEQUENCES.contains(&kind) => {
                for inner in named(target) {
                    self.delete_target(inner, statement);
                }
            }
            _ => self.expression(target, Mode::Value),
        }
    }

    fn delete_name(&mut self, name: Node<'tree>, statement: Node<'tree>) {
        let text = self.text(name);
        self.model.resolve_delete(text, name.start_position());
        let conditional = crate::lint::python::ast::statement_ancestors(statement)
            .any(|ancestor| CONDITIONAL_PARENTS.contains(&ancestor.kind()))
            || in_handler(statement);
        if conditional {
            return;
        }
        let site = Site {
            at: name.start_position(),
            node: Some(name),
            statement: Some(statement),
        };
        let id = self.model.push_binding(BindingKind::Deletion, site);
        self.model.insert(self.model.scope, text, id);
    }

    fn handler(&mut self, handler: Node<'tree>) {
        let value = handler.child_by_field_name(VALUE_FIELD);
        let alias = value.and_then(|value| value.child_by_field_name(crate::lint::python::ast::ALIAS_FIELD));
        let name = alias.and_then(|alias| named(alias).into_iter().find(|inner| inner.kind() == IDENTIFIER));
        let bound = name.map(|bound| {
            let previous = match self.model.lookup(self.text(bound)) {
                Resolution::Binding(id) => Some(id),
                Resolution::Builtin | Resolution::Implicit | Resolution::Unresolved => None,
            };
            let store = Store::plain(BindingKind::BoundException, handler);
            (bound, previous, self.bind_outside_branch(bound, store))
        });
        let exception_type = match (value, alias) {
            (Some(value), Some(_)) => named(value).first().copied(),
            (value, None) => value,
            (None, Some(_)) => None,
        };
        if let Some(exception_type) = exception_type {
            self.expression(exception_type, Mode::Value);
            let aliases = handler_alias_reports(&self.model, self.source, exception_type);
            self.reports.extend(aliases);
        }
        if let Some(blind) = blind_except(&self.model, self.source, handler) {
            self.reports.push(Report::at(PythonRule::BLE001, blind, ""));
        }
        if is_silenced_exception(&self.model, self.source, handler, PASS) {
            self.reports.push(Report::at(PythonRule::S110, handler, ""));
        }
        if is_silenced_exception(&self.model, self.source, handler, CONTINUE) {
            self.reports.push(Report::at(PythonRule::S112, handler, ""));
        }
        self.clause_body(handler);
        if let Some((bound_name, previous, id)) = bound {
            self.unused_exception(bound_name, id, value);
            self.bind_outside_branch(
                bound_name,
                Store::plain(BindingKind::UnboundException(previous), handler),
            );
        }
    }

    fn bind_outside_branch(&mut self, name: Node<'tree>, store: Store<'tree>) -> usize {
        let branch = self.model.branch;
        self.model.branch = self.model.parent_branch();
        let id = self.bind_name(name, store);
        self.model.branch = branch;
        id
    }
}

fn in_handler(statement: Node<'_>) -> bool {
    crate::lint::python::ast::statement_ancestors(statement)
        .take_while(|ancestor| !matches!(ancestor.kind(), FUNCTION | CLASS))
        .any(|ancestor| ancestor.kind() == EXCEPT)
}
