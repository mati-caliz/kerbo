use tree_sitter::Node;

use super::aliases::raise_reports;
use super::compound_checks::compound_reports;
use super::effects::useless_expression;
use super::model::{BindingKind, GLOBAL, MODULE_SCOPE, NAME_ERROR, NONLOCAL, ScopeKind, Site};
use super::qualified::is_builtin_named;
use super::targets::Store;
use super::typecheck::TypeChecks;
use super::{Mode, Visitor};
use crate::lint::python::Report;
use crate::lint::python::ast::text;
use crate::lint::python::ast::{
    ASSIGNMENT, AUGMENTED_ASSIGNMENT, BLOCK, BODY_FIELD, CLASS, COMMENT, DECORATED, EXPRESSION_STATEMENT,
    FUNCTION, FUTURE_IMPORT, IDENTIFIER, IMPORT, IMPORT_FROM, LEFT_FIELD, RIGHT_FIELD, TUPLE, VALUE_FIELD,
    fields, named,
};
use crate::lint::python::semantic::boundary::Boundary;
use crate::lint::python::semantic::definitions::Definitions;
use crate::lint::python::semantic::expressions::Expressions;
use crate::lint::python::semantic::imports::Imports;
use crate::lint::python::semantic::patterns::Patterns;
use crate::lint::python::semantic::targets::Targets;
use crate::rules::PythonRule;

const IF: &str = "if_statement";
const FOR: &str = "for_statement";
const WHILE: &str = "while_statement";
const TRY: &str = "try_statement";
const RAISE: &str = "raise_statement";
const WITH: &str = "with_statement";
const MATCH: &str = "match_statement";
const DELETE: &str = "delete_statement";
const GLOBAL_STATEMENT: &str = "global_statement";
const NONLOCAL_STATEMENT: &str = "nonlocal_statement";
const TYPE_ALIAS: &str = "type_alias_statement";
const EXCEPT: &str = "except_clause";
const ELSE: &str = "else_clause";
const FINALLY: &str = "finally_clause";
const WITH_CLAUSE: &str = "with_clause";
const AS_PATTERN: &str = "as_pattern";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const NAME_ERROR_TYPE: &str = "NameError";
const CLASS_CELL: &str = "__class__";
const PARENTHESIZED_ITEMS: [&str; 2] = [crate::lint::python::ast::PARENTHESIZED, TUPLE];
const GENERIC_TYPE: &str = "generic_type";

pub trait Statements<'tree> {
    fn block(&mut self, node: Node<'tree>);
    fn statement(&mut self, statement: Node<'tree>);
    fn loads(&mut self, node: Node<'tree>);
    fn expression_statement(&mut self, statement: Node<'tree>);
    fn if_statement(&mut self, statement: Node<'tree>);
    fn branch_body(&mut self, body: Option<Node<'tree>>);
    fn clause_body(&mut self, clause: Node<'tree>);
    fn for_statement(&mut self, statement: Node<'tree>);
    fn while_statement(&mut self, statement: Node<'tree>);
    fn loop_rest(&mut self, statement: Node<'tree>);
    fn try_statement(&mut self, statement: Node<'tree>);
    fn handled_by(&self, clauses: &[Node<'tree>]) -> u8;
    fn with_statement(&mut self, statement: Node<'tree>);
    fn with_item(&mut self, value: Node<'tree>, statement: Node<'tree>);
    fn delete(&mut self, statement: Node<'tree>);
    fn global(&mut self, statement: Node<'tree>);
    fn nonlocal(&mut self, statement: Node<'tree>);
    fn type_alias(&mut self, statement: Node<'tree>);
}

impl<'tree> Statements<'tree> for Visitor<'tree> {
    fn block(&mut self, node: Node<'tree>) {
        for statement in named(node) {
            self.statement(statement);
        }
    }

    fn statement(&mut self, statement: Node<'tree>) {
        self.track_boundaries(statement);
        self.reports
            .extend(compound_reports(&self.model, self.source, statement));
        match statement.kind() {
            EXPRESSION_STATEMENT => self.expression_statement(statement),
            IMPORT => self.import(statement),
            IMPORT_FROM => self.import_from(statement),
            FUTURE_IMPORT => self.future_import(statement),
            IF => self.if_statement(statement),
            FOR => self.for_statement(statement),
            WHILE => self.while_statement(statement),
            TRY => self.try_statement(statement),
            WITH => self.with_statement(statement),
            MATCH => self.match_statement(statement),
            FUNCTION | CLASS | DECORATED => self.definition(statement),
            DELETE => self.delete(statement),
            GLOBAL_STATEMENT => self.global(statement),
            NONLOCAL_STATEMENT => self.nonlocal(statement),
            TYPE_ALIAS => self.type_alias(statement),
            COMMENT => {}
            RAISE => {
                self.reports
                    .extend(raise_reports(&self.model, self.source, statement));
                self.loads(statement);
            }
            _ => self.loads(statement),
        }
    }

    fn loads(&mut self, node: Node<'tree>) {
        for child in named(node) {
            self.expression(child, Mode::Value);
        }
    }

    fn expression_statement(&mut self, statement: Node<'tree>) {
        let is_expression = named(statement)
            .iter()
            .all(|child| child.kind() != ASSIGNMENT && child.kind() != AUGMENTED_ASSIGNMENT);
        if is_expression {
            let is_builtin =
                |name: Node<'_>| is_builtin_named(&self.model, name, self.source, text(name, self.source));
            if let Some(value) = useless_expression(statement, self.source, &is_builtin) {
                self.reports.push(Report::at(PythonRule::B018, value, ""));
            }
        }
        for child in named(statement) {
            match child.kind() {
                ASSIGNMENT => self.assignment(child, statement),
                AUGMENTED_ASSIGNMENT => self.augmented_assignment(child, statement),
                _ => self.expression(child, Mode::Value),
            }
        }
    }

    fn if_statement(&mut self, statement: Node<'tree>) {
        self.reports.extend(
            TypeChecks {
                model: &self.model,
                source: self.source,
            }
            .reports(statement),
        );
        if let Some(condition) = statement.child_by_field_name(CONDITION_FIELD) {
            self.expression(condition, Mode::Value);
        }
        self.branch_body(statement.child_by_field_name(CONSEQUENCE_FIELD));
        for clause in fields(statement, ALTERNATIVE_FIELD) {
            self.model.push_branch();
            if let Some(condition) = clause.child_by_field_name(CONDITION_FIELD) {
                self.expression(condition, Mode::Value);
            }
            self.clause_body(clause);
            self.model.pop_branch();
        }
    }

    fn branch_body(&mut self, body: Option<Node<'tree>>) {
        self.model.push_branch();
        if let Some(body) = body {
            self.block(body);
        }
        self.model.pop_branch();
    }

    fn clause_body(&mut self, clause: Node<'tree>) {
        for child in named(clause).into_iter().filter(|child| child.kind() == BLOCK) {
            self.block(child);
        }
    }

    fn for_statement(&mut self, statement: Node<'tree>) {
        if let Some(iterable) = statement.child_by_field_name(RIGHT_FIELD) {
            self.expression(iterable, Mode::Value);
        }
        if let Some(target) = statement.child_by_field_name(LEFT_FIELD) {
            self.store(target, Store::plain(BindingKind::LoopVariable, statement));
        }
        self.loop_rest(statement);
    }

    fn while_statement(&mut self, statement: Node<'tree>) {
        if let Some(condition) = statement.child_by_field_name(CONDITION_FIELD) {
            self.expression(condition, Mode::Value);
        }
        self.loop_rest(statement);
    }

    fn loop_rest(&mut self, statement: Node<'tree>) {
        if let Some(body) = statement.child_by_field_name(BODY_FIELD) {
            self.block(body);
        }
        for clause in named(statement).into_iter().filter(|child| child.kind() == ELSE) {
            self.clause_body(clause);
        }
    }

    fn try_statement(&mut self, statement: Node<'tree>) {
        let clauses = named(statement);
        let handled = self.handled_by(&clauses);
        self.model.push_branch();
        let body_branch = self.model.branch;
        self.model.handled.push(handled);
        if let Some(body) = statement.child_by_field_name(BODY_FIELD) {
            self.block(body);
        }
        self.model.handled.pop();
        self.model.pop_branch();
        for handler in clauses.iter().filter(|clause| clause.kind() == EXCEPT) {
            self.model.push_branch();
            self.handler(*handler);
            self.model.pop_branch();
        }
        self.model.branch = body_branch;
        for clause in clauses.iter().filter(|clause| clause.kind() == ELSE) {
            self.clause_body(*clause);
        }
        self.model.pop_branch();
        for clause in clauses.iter().filter(|clause| clause.kind() == FINALLY) {
            self.model.push_branch();
            self.clause_body(*clause);
            self.model.pop_branch();
        }
    }

    fn handled_by(&self, clauses: &[Node<'tree>]) -> u8 {
        let types = clauses
            .iter()
            .filter(|clause| clause.kind() == EXCEPT)
            .filter_map(|clause| clause.child_by_field_name(VALUE_FIELD))
            .map(|value| {
                if value.kind() == AS_PATTERN {
                    named(value).first().copied().unwrap_or(value)
                } else {
                    value
                }
            })
            .flat_map(|value| {
                if value.kind() == TUPLE {
                    named(value)
                } else {
                    vec![value]
                }
            });
        let mut handled = 0;
        for exception in types {
            if is_builtin_named(&self.model, exception, self.source, NAME_ERROR_TYPE) {
                handled |= NAME_ERROR;
            }
        }
        handled
    }

    fn with_statement(&mut self, statement: Node<'tree>) {
        let clause = named(statement)
            .into_iter()
            .find(|child| child.kind() == WITH_CLAUSE);
        let values = clause
            .map(named)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.child_by_field_name(VALUE_FIELD))
            .flat_map(with_items);
        for value in values {
            self.with_item(value, statement);
        }
        self.model.push_branch();
        if let Some(body) = statement.child_by_field_name(BODY_FIELD) {
            self.block(body);
        }
        self.model.pop_branch();
    }

    fn with_item(&mut self, value: Node<'tree>, statement: Node<'tree>) {
        if value.kind() != AS_PATTERN {
            self.expression(value, Mode::Value);
            return;
        }
        if let Some(context) = named(value).first() {
            self.expression(*context, Mode::Value);
        }
        if let Some(target) = value.child_by_field_name(crate::lint::python::ast::ALIAS_FIELD) {
            let unpacked = named(target).first().is_some_and(|inner| inner.kind() == TUPLE);
            let store = Store::plain(BindingKind::WithItem, statement).unpacked(unpacked);
            for inner in named(target) {
                self.store(inner, store);
            }
        }
    }

    fn delete(&mut self, statement: Node<'tree>) {
        for target in named(statement) {
            self.delete_target(target, statement);
        }
    }

    fn global(&mut self, statement: Node<'tree>) {
        if self.model.current().kind == ScopeKind::Module {
            return;
        }
        for name in named(statement)
            .into_iter()
            .filter(|name| name.kind() == IDENTIFIER)
        {
            let text = self.text(name);
            let module = self.model.scope_of(MODULE_SCOPE).names.get(text).copied();
            let site = Site {
                at: name.start_position(),
                node: Some(name),
                statement: Some(statement),
            };
            let id = self.model.push_binding(BindingKind::Global(module), site);
            self.model.flag(id, GLOBAL);
            self.model.insert(self.model.scope, text, id);
        }
    }

    fn nonlocal(&mut self, statement: Node<'tree>) {
        if self.model.current().kind == ScopeKind::Module {
            return;
        }
        for name in named(statement)
            .into_iter()
            .filter(|name| name.kind() == IDENTIFIER)
        {
            let text = self.text(name);
            let enclosing = self.model.ancestors(self.model.scope).skip(1).find_map(|scope| {
                let found = self.model.scope_of(scope);
                (!matches!(found.kind, ScopeKind::Module | ScopeKind::Class))
                    .then(|| found.names.get(text).copied())
                    .flatten()
            });
            let in_method = self
                .model
                .ancestors(self.model.scope)
                .any(|scope| self.model.scope_of(scope).kind == ScopeKind::Class);
            if enclosing.is_none() && !(text == CLASS_CELL && in_method) {
                continue;
            }
            if let Some(enclosing) = enclosing {
                self.model.reference(enclosing, name.start_position());
            }
            let site = Site {
                at: name.start_position(),
                node: Some(name),
                statement: Some(statement),
            };
            let id = self.model.push_binding(BindingKind::Nonlocal(enclosing), site);
            self.model.flag(id, NONLOCAL);
            self.model.insert(self.model.scope, text, id);
        }
    }

    fn type_alias(&mut self, statement: Node<'tree>) {
        let target = statement
            .child_by_field_name(LEFT_FIELD)
            .and_then(|target| named(target).first().copied());
        let is_alias = target.is_some_and(|target| matches!(target.kind(), IDENTIFIER | GENERIC_TYPE));
        if !is_alias {
            self.loads(statement);
            return;
        }
        let (name, parameters) = match target {
            Some(generic) if generic.kind() == GENERIC_TYPE => {
                let parts = named(generic);
                (parts.first().copied(), parts.get(1).copied())
            }
            other => (other, None),
        };
        if let Some(name) = name.filter(|name| name.kind() == IDENTIFIER) {
            self.store(name, Store::plain(BindingKind::Assignment, statement));
        }
        self.model.push_scope(ScopeKind::Type);
        if let Some(parameters) = parameters {
            self.bind_type_parameters(parameters, statement);
        }
        if let Some(value) = statement.child_by_field_name(RIGHT_FIELD) {
            self.deferred.annotations.push(self.snapshot(value));
        }
        self.model.pop_scope();
    }
}

fn with_items(value: Node<'_>) -> Vec<Node<'_>> {
    if PARENTHESIZED_ITEMS.contains(&value.kind()) && contains_as_pattern(value) {
        named(value).into_iter().flat_map(with_items).collect()
    } else {
        vec![value]
    }
}

fn contains_as_pattern(value: Node<'_>) -> bool {
    named(value).iter().any(|inner| {
        inner.kind() == AS_PATTERN
            || (PARENTHESIZED_ITEMS.contains(&inner.kind()) && contains_as_pattern(*inner))
    })
}
