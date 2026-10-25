use tree_sitter::Node;

use super::boolean_chains::boolean_chain_reports;
use super::calls::{call_reports, is_first_element_subscript};
use super::comparisons::comparison_reports;
use super::conversions::conversion_reports;
use super::if_blocks::dict_get_expression;
use super::isinstance_calls::{BOOLEAN_OPERATOR, duplicate_isinstance_reports};
use super::literals::is_native_literal_call;
use super::model::{BindingKind, Model, ScopeKind};
use super::pep::is_pep585_generic;
use super::qualified::{is_builtin_named, is_typing, qualified_name};
use super::redundant_tests::negated_operands;
use super::short_circuit::short_circuit_reports;
use super::targets::Store;
use super::type_hints::{Hints, type_hint_reports};
use super::type_names::is_type_name;
use super::type_unions::{Deferral, redundant_type_union, runtime_string_union};
use super::version_checks::{is_six_py3, version_subscript};
use super::{Mode, Visitor};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ATTRIBUTE, CALL, COMPARISON, CONCATENATED_STRING, IDENTIFIER, INTERPOLATION, KEYWORD_ARGUMENT, LAMBDA,
    NAME_FIELD, OBJECT_FIELD, OPERATORS_FIELD, STRING, SUBSCRIPT, VALUE_FIELD, fields, named, text,
};
use crate::lint::python::semantic::annotations::Annotations;
use crate::lint::python::semantic::annotations::typing::TypingCalls;
use crate::lint::python::semantic::definitions::Definitions;
use crate::lint::python::semantic::targets::Targets;
use crate::rules::PythonRule;

const NAMED_EXPRESSION: &str = "named_expression";
const GENERIC_TYPE: &str = "generic_type";
const CONDITIONAL: &str = "conditional_expression";
const MEMBER_TYPE: &str = "member_type";
const COMPREHENSIONS: [&str; 4] = [
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
];
const LOCALS: &str = "locals";
const EXEC: &str = "exec";
const UTC_ALIAS: &str = "datetime.timezone.utc";
const LOGGING_WARN: &str = "logging.WARN";
const TEXT_ALIAS: &str = "Text";
const REGEX_FLAG_ALIASES: [&str; 8] = ["re.A", "re.I", "re.L", "re.M", "re.S", "re.T", "re.U", "re.X"];
const ARGUMENT_LIST: &str = "argument_list";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const DICT_BUILTIN: &str = "dict";
const SET_BUILTIN: &str = "set";
const LITERAL_SEQUENCES: [&str; 2] = ["list", "tuple"];
const COLLECTION_BUILTINS: [&str; 3] = [DICT_BUILTIN, "list", "tuple"];
const TYPE_BUILTIN: &str = "type";
const EQUALITY: [&str; 2] = ["==", "!="];
const DTYPE: &str = "dtype";
const NUMPY_DTYPE: &str = "numpy.dtype";

pub trait Expressions<'tree> {
    fn expression(&mut self, node: Node<'tree>, mode: Mode);
    fn union_reports(&mut self, node: Node<'tree>, mode: Mode);
    fn children(&mut self, node: Node<'tree>, mode: Mode);
    fn named_expression(&mut self, node: Node<'tree>, mode: Mode);
    fn string(&mut self, node: Node<'tree>, mode: Mode);
    fn call(&mut self, call: Node<'tree>, mode: Mode);
    fn type_comparison(&mut self, comparison: Node<'tree>);
    fn upgradable_reference(&mut self, node: Node<'tree>);
    fn is_type(&self, node: Node<'_>) -> bool;
    fn is_dtype(&self, node: Node<'_>) -> bool;
}

impl<'tree> Expressions<'tree> for Visitor<'tree> {
    fn expression(&mut self, node: Node<'tree>, mode: Mode) {
        let stub_type = self.file.stub && mode == Mode::TypeDefinition && !self.model.forward_reference;
        if stub_type && !matches!(node.kind(), STRING | CONCATENATED_STRING) {
            self.deferred.annotations.push(self.snapshot(node));
            return;
        }
        self.reports
            .extend(type_hint_reports(&self.model, self.source, node, mode));
        self.union_reports(node, mode);
        match node.kind() {
            IDENTIFIER => {
                self.model.resolve_load(self.text(node), node.start_position());
                self.upgradable_reference(node);
            }
            ATTRIBUTE => {
                if let Some(object) = node.child_by_field_name(OBJECT_FIELD) {
                    self.expression(object, mode);
                }
                self.upgradable_reference(node);
            }
            SUBSCRIPT => {
                if is_first_element_subscript(&self.model, self.source, node) {
                    self.reports.push(Report::at(PythonRule::RUF015, node, ""));
                }
                self.reports
                    .extend(version_subscript(&self.model, self.source, node));
                self.subscript(node, mode);
            }
            GENERIC_TYPE => self.generic_type(node, mode),
            MEMBER_TYPE => {
                if let Some(object) = named(node).first() {
                    self.expression(*object, mode);
                }
            }
            CALL => self.call(node, mode),
            STRING | CONCATENATED_STRING => self.string(node, mode),
            LAMBDA => self.lambda(node),
            NAMED_EXPRESSION => self.named_expression(node, mode),
            KEYWORD_ARGUMENT => {
                if let Some(value) = node.child_by_field_name(VALUE_FIELD) {
                    self.expression(value, mode);
                }
            }
            COMPARISON => {
                self.reports
                    .extend(comparison_reports(&self.model, self.source, node));
                self.type_comparison(node);
                self.children(node, mode);
            }
            CONDITIONAL => {
                self.reports
                    .extend(dict_get_expression(&self.model, self.source, node));
                let mut parts = named(node);
                if parts.len() > 1 {
                    parts.swap(0, 1);
                }
                for part in parts {
                    self.expression(part, mode);
                }
            }
            BOOLEAN_OPERATOR => {
                self.reports
                    .extend(duplicate_isinstance_reports(&self.model, self.source, node));
                self.reports
                    .extend(short_circuit_reports(&self.model, self.source, node));
                self.reports
                    .extend(boolean_chain_reports(&self.model, self.source, node));
                self.reports
                    .extend(negated_operands(&self.model, self.source, node));
                self.children(node, mode);
            }
            kind if COMPREHENSIONS.contains(&kind) => self.comprehension(node),
            _ => self.children(node, mode),
        }
    }

    // PYI055 y TC010, que dependen de si la anotación se evalúa.
    fn union_reports(&mut self, node: Node<'tree>, mode: Mode) {
        let hints = Hints {
            model: &self.model,
            source: self.source,
        };
        let deferred = self.boundaries.future_annotations() || self.file.stub;
        self.reports
            .extend(redundant_type_union(hints, node, mode, deferred));
        let deferral = Deferral {
            always: self.file.stub || self.model.forward_reference,
            future_annotations: self.boundaries.future_annotations(),
        };
        self.reports
            .extend(runtime_string_union(hints, node, mode, deferral));
    }

    fn children(&mut self, node: Node<'tree>, mode: Mode) {
        for child in named(node) {
            self.expression(child, mode);
        }
    }

    fn named_expression(&mut self, node: Node<'tree>, mode: Mode) {
        if let Some(value) = node.child_by_field_name(VALUE_FIELD) {
            self.expression(value, mode);
        }
        if let Some(name) = node.child_by_field_name(NAME_FIELD) {
            let statement = super::definitions::enclosing_statement(node).unwrap_or(node);
            self.bind_name(name, Store::plain(BindingKind::NamedExpression, statement));
        }
    }

    fn string(&mut self, node: Node<'tree>, mode: Mode) {
        if mode.defines_types() {
            self.deferred.strings.push(self.snapshot(node));
            return;
        }
        let parts = if node.kind() == STRING {
            vec![node]
        } else {
            named(node)
        };
        self.reports
            .extend(conversion_reports(&self.model, self.source, &parts));
        for interpolation in parts
            .iter()
            .flat_map(|part| named(*part))
            .filter(|child| child.kind() == INTERPOLATION)
        {
            self.children(interpolation, Mode::Value);
        }
    }

    fn call(&mut self, call: Node<'tree>, mode: Mode) {
        let function = call.child_by_field_name(crate::lint::python::ast::FUNCTION_FIELD);
        if let Some(function) = function {
            self.expression(function, mode);
            if is_builtin_named(&self.model, function, self.source, EXEC) {
                self.reports.push(Report::at(PythonRule::S102, function, ""));
            }
            if is_builtin_named(&self.model, function, self.source, LOCALS)
                && self.model.current().kind == ScopeKind::Function
                && let Some(scope) = self.model.current_mut()
            {
                scope.uses_locals = true;
            }
        }
        self.reports.extend(call_reports(&self.model, self.source, call));
        let Some(arguments) = call.child_by_field_name(crate::lint::python::ast::ARGUMENTS_FIELD) else {
            return;
        };
        if let Some(function) = function
            && let Some(collection) = unnecessary_collection(&self.model, self.source, function, arguments)
        {
            self.reports.push(Report::at(PythonRule::C408, call, collection));
        }
        if let Some(function) = function
            && is_literal_set_call(&self.model, self.source, function, arguments)
        {
            self.reports.push(Report::at(PythonRule::C405, call, ""));
        }
        if let Some(function) = function
            && is_native_literal_call(&self.model, self.source, function, arguments)
        {
            self.reports.push(Report::at(PythonRule::UP018, call, ""));
        }
        if COMPREHENSIONS.contains(&arguments.kind()) {
            self.comprehension(arguments);
            return;
        }
        let callee = function.and_then(|function| qualified_name(&self.model, function, self.source));
        self.typing_call(arguments, callee.as_deref());
    }

    // UP006: `typing.List` y compañía donde alcanza con `list`; UP017: `datetime.timezone.utc`; LOG009:
    // `logging.WARN`, el alias sin documentar de `logging.WARNING`.
    fn upgradable_reference(&mut self, node: Node<'tree>) {
        let Some(name) = qualified_name(&self.model, node, self.source) else {
            return;
        };
        if is_pep585_generic(&name) {
            self.reports.push(Report::at(PythonRule::UP006, node, ""));
        } else if name == UTC_ALIAS {
            self.reports.push(Report::at(PythonRule::UP017, node, ""));
        } else if REGEX_FLAG_ALIASES.contains(&name.as_str()) {
            self.reports.push(Report::at(PythonRule::FURB167, node, ""));
        } else if is_typing(&name, TEXT_ALIAS) {
            self.reports.push(Report::at(PythonRule::UP019, node, ""));
        } else if is_six_py3(&name) {
            self.reports.push(Report::at(PythonRule::YTT202, node, ""));
        } else if name == LOGGING_WARN {
            self.reports.push(Report::at(PythonRule::LOG009, node, ""));
        }
    }

    fn type_comparison(&mut self, comparison: Node<'tree>) {
        let operands = named(comparison);
        let operators = fields(comparison, OPERATORS_FIELD);
        for (index, operator) in operators.iter().enumerate() {
            if !EQUALITY.contains(&self.text(*operator)) {
                continue;
            }
            let (Some(left), Some(right)) = (operands.get(index), operands.get(index.saturating_add(1)))
            else {
                continue;
            };
            let typed = self.is_type(*left) || self.is_type(*right);
            if typed && !self.is_dtype(*left) && !self.is_dtype(*right) {
                self.reports.push(Report::at(PythonRule::E721, comparison, ""));
            }
        }
    }

    fn is_type(&self, node: Node<'_>) -> bool {
        match node.kind() {
            CALL => node
                .child_by_field_name(crate::lint::python::ast::FUNCTION_FIELD)
                .is_some_and(|function| is_builtin_named(&self.model, function, self.source, TYPE_BUILTIN)),
            IDENTIFIER => {
                let name = text(node, self.source);
                is_type_name(name) && is_builtin_named(&self.model, node, self.source, name)
            }
            _ => false,
        }
    }

    fn is_dtype(&self, node: Node<'_>) -> bool {
        match node.kind() {
            CALL => node
                .child_by_field_name(crate::lint::python::ast::FUNCTION_FIELD)
                .and_then(|function| qualified_name(&self.model, function, self.source))
                .is_some_and(|name| name == NUMPY_DTYPE),
            ATTRIBUTE => node
                .child_by_field_name(crate::lint::python::ast::ATTRIBUTE_FIELD)
                .is_some_and(|attribute| text(attribute, self.source) == DTYPE),
            _ => false,
        }
    }
}

// C408: `dict()`, `list()` o `tuple()` del builtin, sin posicionales (y `dict` sin `**`), en vez del
// literal.
fn unnecessary_collection(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    arguments: Node<'_>,
) -> Option<&'static str> {
    if arguments.kind() != ARGUMENT_LIST {
        return None;
    }
    let items = named(arguments);
    let keywords = items
        .iter()
        .filter(|item| item.kind() == KEYWORD_ARGUMENT)
        .count();
    let splats = items
        .iter()
        .filter(|item| item.kind() == DICTIONARY_SPLAT)
        .count();
    if items.len() != keywords.saturating_add(splats) {
        return None;
    }
    COLLECTION_BUILTINS
        .iter()
        .find(|name| is_builtin_named(model, function, source, name))
        .copied()
        .filter(|name| {
            if *name == DICT_BUILTIN {
                splats == 0
            } else {
                items.is_empty()
            }
        })
}

// C405: `set([...])` o `set((...))` con el `set` del builtin, en vez del literal `{...}`.
fn is_literal_set_call(model: &Model<'_>, source: &[u8], function: Node<'_>, arguments: Node<'_>) -> bool {
    let is_set_name = function.kind() == IDENTIFIER && text(function, source) == SET_BUILTIN;
    let single_literal = arguments.kind() == ARGUMENT_LIST
        && matches!(named(arguments).as_slice(), [argument] if LITERAL_SEQUENCES.contains(&crate::lint::python::ast::unparenthesized(*argument).kind()));
    is_set_name && single_literal && is_builtin_named(model, function, source, SET_BUILTIN)
}
