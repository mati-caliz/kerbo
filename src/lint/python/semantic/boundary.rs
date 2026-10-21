use tree_sitter::Node;

use super::Visitor;
use super::model::{MODULE_SCOPE, ScopeKind};
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ALIASED_IMPORT, ASSIGNMENT, AUGMENTED_ASSIGNMENT, CALL, EXPRESSION_STATEMENT, FUNCTION_FIELD,
    FUTURE_IMPORT, IDENTIFIER, IMPORT, IMPORT_FROM, LEFT_FIELD, MODULE, NAME_FIELD, RIGHT_FIELD, SUBSCRIPT,
    TYPE_FIELD, VALUE_FIELD, is_bytes, is_f_string, is_string, named,
};
use crate::rules::PythonRule;

const DOCSTRING: u8 = 1;
const FUTURES: u8 = 2;
const IMPORTS: u8 = 4;
const FUTURE_ANNOTATIONS: u8 = 8;
const NESTING: [&str; 4] = [
    "try_statement",
    "if_statement",
    "with_statement",
    "match_statement",
];
const DELETE: &str = "delete_statement";
const DUNDER: &str = "__";
const ANNOTATIONS_FEATURE: &str = "annotations";
const SYS_PATH: &str = "sys.path";
const OS_ENVIRON: &str = "os.environ";
const SYS_PATH_METHODS: [&str; 8] = [
    "append", "insert", "extend", "remove", "pop", "clear", "reverse", "sort",
];
const ENVIRON_METHODS: [&str; 5] = ["update", "pop", "clear", "setdefault", "popitem"];
const OS_FUNCTIONS: [&str; 2] = ["os.putenv", "os.unsetenv"];
const IMPORT_OR_SKIP: &str = "pytest.importorskip";
const ACTIVATIONS: [&str; 2] = ["matplotlib.use", IMPORT_OR_SKIP];

#[derive(Debug, Clone, Copy, Default)]
pub struct Boundaries {
    flags: u8,
}

impl Boundaries {
    pub const fn future_annotations(self) -> bool {
        self.flags & FUTURE_ANNOTATIONS != 0
    }

    const fn seen(self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

pub trait Boundary {
    fn track_boundaries(&mut self, statement: Node<'_>);
    fn track_future_import(&mut self, statement: Node<'_>);
    fn import_out_of_place(&mut self, statement: Node<'_>);
    fn allowed_before_imports(&self, statement: Node<'_>) -> bool;
    fn is_setup_statement(&self, statement: Node<'_>) -> bool;
    fn is_setup_call(&self, call: Node<'_>) -> bool;
    fn names_target(&self, target: Node<'_>, expected: &str) -> bool;
}

impl Boundary for Visitor<'_> {
    fn track_boundaries(&mut self, statement: Node<'_>) {
        if self.model.current().kind != ScopeKind::Module {
            return;
        }
        match statement.kind() {
            EXPRESSION_STATEMENT
                if !self.boundaries.seen(DOCSTRING) && is_docstring(statement, self.source) =>
            {
                self.boundaries.flags |= DOCSTRING;
            }
            FUTURE_IMPORT => self.track_future_import(statement),
            IMPORT_FROM => self.boundaries.flags |= DOCSTRING | FUTURES,
            IMPORT => {
                self.boundaries.flags |= DOCSTRING | FUTURES;
            }
            _ => {
                self.boundaries.flags |= DOCSTRING | FUTURES;
                if !self.boundaries.seen(IMPORTS) && !self.allowed_before_imports(statement) {
                    self.boundaries.flags |= IMPORTS;
                }
            }
        }
    }

    fn track_future_import(&mut self, statement: Node<'_>) {
        self.boundaries.flags |= DOCSTRING;
        if self.boundaries.seen(FUTURES) {
            let subject = self.text(statement);
            self.reports
                .push(Report::at(PythonRule::F404, statement, subject));
        }
        let features = named(statement).into_iter().map(|name| {
            if name.kind() == ALIASED_IMPORT {
                name.child_by_field_name(NAME_FIELD).unwrap_or(name)
            } else {
                name
            }
        });
        if features
            .into_iter()
            .any(|name| self.text(name) == ANNOTATIONS_FEATURE)
        {
            self.boundaries.flags |= FUTURE_ANNOTATIONS;
        }
    }

    fn import_out_of_place(&mut self, statement: Node<'_>) {
        let top_level = self.model.scope == MODULE_SCOPE
            && statement.parent().is_some_and(|parent| parent.kind() == MODULE);
        if self.boundaries.seen(IMPORTS) && top_level {
            self.reports.push(Report::at(PythonRule::E402, statement, ""));
        }
    }

    fn allowed_before_imports(&self, statement: Node<'_>) -> bool {
        std::iter::successors(Some(statement), Node::parent).any(|node| NESTING.contains(&node.kind()))
            || is_dunder_assignment(statement, self.source)
            || self.is_setup_statement(statement)
    }

    fn is_setup_statement(&self, statement: Node<'_>) -> bool {
        if statement.kind() == DELETE {
            return named(statement)
                .iter()
                .any(|target| self.names_target(*target, OS_ENVIRON));
        }
        let Some(inner) = named(statement).first().copied() else {
            return false;
        };
        match inner.kind() {
            CALL => self.is_setup_call(inner),
            ASSIGNMENT | AUGMENTED_ASSIGNMENT => {
                inner.child_by_field_name(LEFT_FIELD).is_some_and(|target| {
                    self.names_target(target, SYS_PATH) || self.names_target(target, OS_ENVIRON)
                })
            }
            _ => false,
        }
    }

    fn is_setup_call(&self, call: Node<'_>) -> bool {
        let Some(name) = call
            .child_by_field_name(FUNCTION_FIELD)
            .and_then(|function| qualified_name(&self.model, function, self.source))
        else {
            return false;
        };
        let method_of = |object: &str, methods: &[&str]| {
            name.strip_prefix(object)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|method| methods.contains(&method))
        };
        method_of(SYS_PATH, &SYS_PATH_METHODS)
            || method_of(OS_ENVIRON, &ENVIRON_METHODS)
            || OS_FUNCTIONS.contains(&name.as_str())
            || ACTIVATIONS.contains(&name.as_str())
    }

    fn names_target(&self, target: Node<'_>, expected: &str) -> bool {
        let target = if target.kind() == SUBSCRIPT {
            target.child_by_field_name(VALUE_FIELD).unwrap_or(target)
        } else {
            target
        };
        qualified_name(&self.model, target, self.source).is_some_and(|name| name == expected)
    }
}

fn is_docstring(statement: Node<'_>, source: &[u8]) -> bool {
    matches!(named(statement).as_slice(), [only] if is_string(*only) && !is_f_string(*only, source) && !is_bytes(*only, source))
}

fn is_dunder_assignment(statement: Node<'_>, source: &[u8]) -> bool {
    let Some(assignment) = named(statement)
        .first()
        .copied()
        .filter(|inner| inner.kind() == ASSIGNMENT)
    else {
        return false;
    };
    let annotated = assignment.child_by_field_name(TYPE_FIELD).is_some();
    let chained = assignment
        .child_by_field_name(RIGHT_FIELD)
        .is_some_and(|value| value.kind() == ASSIGNMENT);
    assignment
        .child_by_field_name(LEFT_FIELD)
        .filter(|target| target.kind() == IDENTIFIER && (annotated || !chained))
        .is_some_and(|target| {
            let name = crate::lint::python::ast::text(target, source);
            name.len() > DUNDER.len() && name.starts_with(DUNDER) && name.ends_with(DUNDER)
        })
}
