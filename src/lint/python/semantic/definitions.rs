use tree_sitter::Node;

use super::defaults::default_reports;
use super::mutability::Resolver;

use super::model::{BindingKind, MODULE_SCOPE, ScopeKind, Site};
use super::qualified::is_builtin_named;
use super::returns::{unnecessary_return_none, useless_return};
use super::targets::Store;
use super::{Mode, Pending, Visitor};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    BODY_FIELD, CLASS, DECORATED, FUNCTION, IDENTIFIER, LEFT_FIELD, NAME_FIELD, RIGHT_FIELD, TYPE_FIELD,
    VALUE_FIELD, named,
};
use crate::lint::python::semantic::annotations::Annotations;
use crate::lint::python::semantic::expressions::Expressions;
use crate::lint::python::semantic::statements::Statements;
use crate::lint::python::semantic::targets::Targets;
use crate::rules::PythonRule;

const OBJECT_BUILTIN: &str = "object";
const DEFINITION_FIELD: &str = "definition";
const DECORATOR: &str = "decorator";
const PARAMETERS_FIELD: &str = "parameters";
const TYPE_PARAMETERS_FIELD: &str = "type_parameters";
const SUPERCLASSES_FIELD: &str = "superclasses";
const RETURN_TYPE_FIELD: &str = "return_type";
const GLOBAL_STATEMENT: &str = "global_statement";
const CONSTRAINED_TYPE: &str = "constrained_type";
const DEFAULTED: [&str; 2] = ["default_parameter", "typed_default_parameter"];
const TYPED: [&str; 2] = ["typed_parameter", "typed_default_parameter"];
const SPLATS: [&str; 2] = ["list_splat_pattern", "dictionary_splat_pattern"];
const FOR_IN: &str = "for_in_clause";
const IF_CLAUSE: &str = "if_clause";
const FOR_STATEMENT: &str = "for_statement";
const WITH_STATEMENT: &str = "with_statement";
const STATEMENT_SUFFIX: &str = "_statement";

pub trait Definitions<'tree> {
    fn definition(&mut self, statement: Node<'tree>);
    fn function(&mut self, function: Node<'tree>);
    fn class(&mut self, class: Node<'tree>);
    fn type_parameters(&mut self, definition: Node<'tree>);
    fn bind_type_parameters(&mut self, parameters: Node<'tree>, definition: Node<'tree>);
    fn declare_globals(&mut self, body: Option<Node<'tree>>);
    fn declare_global(&mut self, statement: Node<'tree>);
    fn function_body(&mut self, pending: Pending<'tree>);
    fn lambda(&mut self, lambda: Node<'tree>);
    fn lambda_body(&mut self, pending: Pending<'tree>);
    fn parameters(&mut self, parameters: Node<'tree>, owner: Node<'tree>);
    fn comprehension(&mut self, comprehension: Node<'tree>);
}

impl<'tree> Definitions<'tree> for Visitor<'tree> {
    fn definition(&mut self, statement: Node<'tree>) {
        let (decorators, definition) = if statement.kind() == DECORATED {
            let decorators: Vec<Node<'tree>> = named(statement)
                .into_iter()
                .filter(|child| child.kind() == DECORATOR)
                .collect();
            (decorators, statement.child_by_field_name(DEFINITION_FIELD))
        } else {
            (Vec::new(), Some(statement))
        };
        for decorator in &decorators {
            self.loads(*decorator);
        }
        match definition {
            Some(function) if function.kind() == FUNCTION => {
                let returns = unnecessary_return_none(&self.model, self.source, function, &decorators);
                self.reports.extend(returns);
                self.reports.extend(useless_return(self.source, function));
                self.function(function);
            }
            Some(class) if class.kind() == CLASS => self.class(class),
            _ => {}
        }
    }

    fn function(&mut self, function: Node<'tree>) {
        self.declare_globals(function.child_by_field_name(BODY_FIELD));
        self.model.push_scope(ScopeKind::Type);
        self.type_parameters(function);
        let parameters = function
            .child_by_field_name(PARAMETERS_FIELD)
            .map(named)
            .unwrap_or_default();
        let resolver = Resolver {
            model: &self.model,
            source: self.source,
            scope: self.model.scope,
        };
        let defaults = default_reports(&resolver, &parameters, !self.file.stub);
        self.reports.extend(defaults);
        for default in parameters
            .iter()
            .filter(|parameter| DEFAULTED.contains(&parameter.kind()))
            .filter_map(|parameter| parameter.child_by_field_name(VALUE_FIELD))
        {
            self.expression(default, Mode::Value);
        }
        let annotations = parameters
            .iter()
            .filter(|parameter| TYPED.contains(&parameter.kind()))
            .filter_map(|parameter| parameter.child_by_field_name(TYPE_FIELD))
            .chain(function.child_by_field_name(RETURN_TYPE_FIELD));
        for annotation in annotations {
            self.annotation(annotation);
        }
        self.model.push_scope(ScopeKind::Function);
        self.deferred.functions.push(self.snapshot(function));
        self.model.pop_scope();
        self.model.pop_scope();
        if let Some(name) = function.child_by_field_name(NAME_FIELD) {
            self.bind_name(name, Store::plain(BindingKind::Function, function));
        }
    }

    fn class(&mut self, class: Node<'tree>) {
        self.classes.push(self.snapshot(class));
        self.declare_globals(class.child_by_field_name(BODY_FIELD));
        self.model.push_scope(ScopeKind::Type);
        self.type_parameters(class);
        if let Some(arguments) = class.child_by_field_name(SUPERCLASSES_FIELD) {
            self.expression(arguments, Mode::Value);
            for base in named(arguments)
                .into_iter()
                .filter(|base| is_builtin_named(&self.model, *base, self.source, OBJECT_BUILTIN))
            {
                self.reports.push(Report::at(PythonRule::UP004, base, ""));
            }
        }
        self.model.push_scope(ScopeKind::Class);
        if let Some(body) = class.child_by_field_name(BODY_FIELD) {
            self.block(body);
        }
        self.model.pop_scope();
        self.model.pop_scope();
        if let Some(name) = class.child_by_field_name(NAME_FIELD) {
            self.bind_name(name, Store::plain(BindingKind::Class, class));
        }
    }

    fn type_parameters(&mut self, definition: Node<'tree>) {
        if let Some(parameters) = definition.child_by_field_name(TYPE_PARAMETERS_FIELD) {
            self.bind_type_parameters(parameters, definition);
        }
    }

    fn bind_type_parameters(&mut self, parameters: Node<'tree>, definition: Node<'tree>) {
        for parameter in named(parameters) {
            let inner = named(parameter).first().copied().unwrap_or(parameter);
            let parts = if inner.kind() == CONSTRAINED_TYPE {
                named(inner)
            } else {
                vec![inner]
            };
            let Some((first, bounds)) = parts.split_first() else {
                continue;
            };
            let name = std::iter::successors(Some(*first), |node| named(*node).first().copied())
                .find(|node| node.kind() == IDENTIFIER);
            if let Some(name) = name {
                self.bind_name(name, Store::plain(BindingKind::TypeParameter, definition));
            }
            for bound in bounds {
                self.deferred.annotations.push(self.snapshot(*bound));
            }
        }
    }

    fn declare_globals(&mut self, body: Option<Node<'tree>>) {
        let mut pending: Vec<Node<'tree>> = body.into_iter().collect();
        while let Some(node) = pending.pop() {
            let (globals, nested): (Vec<Node<'tree>>, Vec<Node<'tree>>) = named(node)
                .into_iter()
                .filter(|child| !matches!(child.kind(), FUNCTION | CLASS | DECORATED))
                .partition(|child| child.kind() == GLOBAL_STATEMENT);
            pending.extend(nested);
            for global in globals {
                self.declare_global(global);
            }
        }
    }

    fn declare_global(&mut self, statement: Node<'tree>) {
        for name in named(statement)
            .into_iter()
            .filter(|name| name.kind() == IDENTIFIER)
        {
            let text = self.text(name);
            let module = self.model.scope_of(MODULE_SCOPE).names.get(text).copied();
            let bound = module
                .and_then(|id| self.model.binding(id))
                .is_some_and(|binding| !binding.kind.is_unbound());
            if bound {
                continue;
            }
            let site = Site {
                at: name.start_position(),
                node: Some(name),
                statement: Some(statement),
            };
            let id = self.model.push_binding(BindingKind::Assignment, site);
            if let Some(binding) = self.model.bindings.get_mut(id) {
                binding.scope = MODULE_SCOPE;
            }
            self.model.insert(MODULE_SCOPE, text, id);
        }
    }

    fn function_body(&mut self, pending: Pending<'tree>) {
        self.restore(pending);
        let function = pending.node;
        if let Some(parameters) = function.child_by_field_name(PARAMETERS_FIELD) {
            self.parameters(parameters, function);
        }
        if let Some(body) = function.child_by_field_name(BODY_FIELD) {
            self.block(body);
        }
    }

    fn lambda(&mut self, lambda: Node<'tree>) {
        let parameters = lambda
            .child_by_field_name(PARAMETERS_FIELD)
            .map(named)
            .unwrap_or_default();
        let resolver = Resolver {
            model: &self.model,
            source: self.source,
            scope: self.model.scope,
        };
        let defaults = default_reports(&resolver, &parameters, false);
        self.reports.extend(defaults);
        for default in parameters
            .iter()
            .filter(|parameter| DEFAULTED.contains(&parameter.kind()))
            .filter_map(|parameter| parameter.child_by_field_name(VALUE_FIELD))
        {
            self.expression(default, Mode::Value);
        }
        self.model.push_scope(ScopeKind::Lambda);
        self.deferred.lambdas.push(self.snapshot(lambda));
        self.model.pop_scope();
    }

    fn lambda_body(&mut self, pending: Pending<'tree>) {
        self.restore(pending);
        let lambda = pending.node;
        if let Some(parameters) = lambda.child_by_field_name(PARAMETERS_FIELD) {
            self.parameters(parameters, lambda);
        }
        if let Some(body) = lambda.child_by_field_name(BODY_FIELD) {
            self.expression(body, Mode::Value);
        }
    }

    fn parameters(&mut self, parameters: Node<'tree>, owner: Node<'tree>) {
        for parameter in named(parameters) {
            if let Some(name) = parameter_name(parameter) {
                self.bind_name(name, Store::plain(BindingKind::Argument, owner));
            }
        }
    }

    fn comprehension(&mut self, comprehension: Node<'tree>) {
        let clauses: Vec<Node<'tree>> = named(comprehension)
            .into_iter()
            .filter(|clause| matches!(clause.kind(), FOR_IN | IF_CLAUSE))
            .collect();
        let first_iterable = clauses
            .first()
            .filter(|clause| clause.kind() == FOR_IN)
            .and_then(|clause| clause.child_by_field_name(RIGHT_FIELD));
        if let Some(iterable) = first_iterable {
            self.expression(iterable, Mode::Value);
        }
        self.model.push_scope(ScopeKind::Generator);
        let statement = enclosing_statement(comprehension);
        let kind = match statement.map(|statement| statement.kind()) {
            Some(FOR_STATEMENT) => BindingKind::LoopVariable,
            Some(WITH_STATEMENT) => BindingKind::WithItem,
            _ => BindingKind::Assignment,
        };
        for (index, clause) in clauses.iter().enumerate() {
            if clause.kind() == IF_CLAUSE {
                self.loads(*clause);
                continue;
            }
            if index > 0
                && let Some(iterable) = clause.child_by_field_name(RIGHT_FIELD)
            {
                self.expression(iterable, Mode::Value);
            }
            if let Some(target) = clause.child_by_field_name(LEFT_FIELD) {
                self.store(target, Store::plain(kind, statement.unwrap_or(comprehension)));
            }
        }
        if let Some(body) = comprehension.child_by_field_name(BODY_FIELD) {
            self.expression(body, Mode::Value);
        }
        self.model.pop_scope();
    }
}

fn parameter_name(parameter: Node<'_>) -> Option<Node<'_>> {
    match parameter.kind() {
        IDENTIFIER => Some(parameter),
        kind if DEFAULTED.contains(&kind) => parameter
            .child_by_field_name(NAME_FIELD)
            .filter(|name| name.kind() == IDENTIFIER),
        kind if SPLATS.contains(&kind) => named(parameter)
            .into_iter()
            .find(|inner| inner.kind() == IDENTIFIER),
        _ => named(parameter).first().copied().and_then(parameter_name),
    }
}

pub fn enclosing_statement(node: Node<'_>) -> Option<Node<'_>> {
    crate::lint::python::ast::statement_ancestors(node).find(|ancestor| {
        ancestor.kind().ends_with(STATEMENT_SUFFIX) || matches!(ancestor.kind(), FUNCTION | CLASS | DECORATED)
    })
}
