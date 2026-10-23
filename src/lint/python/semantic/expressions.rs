use tree_sitter::Node;

use super::model::{BindingKind, Model, ScopeKind};
use super::qualified::{is_builtin_named, qualified_name};
use super::targets::Store;
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
const TYPE_NAMES: [&str; 92] = [
    "bool",
    "bytearray",
    "bytes",
    "classmethod",
    "complex",
    "dict",
    "enumerate",
    "filter",
    "float",
    "frozenset",
    "int",
    "list",
    "map",
    "memoryview",
    "object",
    "property",
    "range",
    "reversed",
    "set",
    "slice",
    "staticmethod",
    "str",
    "super",
    "tuple",
    "type",
    "zip",
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BaseException",
    "BlockingIOError",
    "BrokenPipeError",
    "BufferError",
    "BytesWarning",
    "ChildProcessError",
    "ConnectionAbortedError",
    "ConnectionError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "DeprecationWarning",
    "EnvironmentError",
    "EOFError",
    "Exception",
    "FileExistsError",
    "FileNotFoundError",
    "FloatingPointError",
    "FutureWarning",
    "GeneratorExit",
    "ImportError",
    "ImportWarning",
    "IndentationError",
    "IndexError",
    "InterruptedError",
    "IOError",
    "IsADirectoryError",
    "KeyboardInterrupt",
    "KeyError",
    "LookupError",
    "MemoryError",
    "ModuleNotFoundError",
    "NameError",
    "NotADirectoryError",
    "NotImplementedError",
    "OSError",
    "OverflowError",
    "PendingDeprecationWarning",
    "PermissionError",
    "ProcessLookupError",
    "RecursionError",
    "ReferenceError",
    "ResourceWarning",
    "RuntimeError",
    "RuntimeWarning",
    "StopAsyncIteration",
    "StopIteration",
    "SyntaxError",
    "SyntaxWarning",
    "SystemError",
    "SystemExit",
    "TabError",
    "TimeoutError",
    "TypeError",
    "UnboundLocalError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeError",
    "UnicodeTranslateError",
    "UnicodeWarning",
    "UserWarning",
    "ValueError",
    "Warning",
    "ZeroDivisionError",
];

pub trait Expressions<'tree> {
    fn expression(&mut self, node: Node<'tree>, mode: Mode);
    fn children(&mut self, node: Node<'tree>, mode: Mode);
    fn named_expression(&mut self, node: Node<'tree>, mode: Mode);
    fn string(&mut self, node: Node<'tree>, mode: Mode);
    fn call(&mut self, call: Node<'tree>, mode: Mode);
    fn type_comparison(&mut self, comparison: Node<'tree>);
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
        match node.kind() {
            IDENTIFIER => {
                self.model.resolve_load(self.text(node), node.start_position());
            }
            ATTRIBUTE => {
                if let Some(object) = node.child_by_field_name(OBJECT_FIELD) {
                    self.expression(object, mode);
                }
            }
            SUBSCRIPT => self.subscript(node, mode),
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
                self.type_comparison(node);
                self.children(node, mode);
            }
            CONDITIONAL => {
                let mut parts = named(node);
                if parts.len() > 1 {
                    parts.swap(0, 1);
                }
                for part in parts {
                    self.expression(part, mode);
                }
            }
            kind if COMPREHENSIONS.contains(&kind) => self.comprehension(node),
            _ => self.children(node, mode),
        }
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
            if is_builtin_named(&self.model, function, self.source, LOCALS)
                && self.model.current().kind == ScopeKind::Function
                && let Some(scope) = self.model.current_mut()
            {
                scope.uses_locals = true;
            }
        }
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
        if COMPREHENSIONS.contains(&arguments.kind()) {
            self.comprehension(arguments);
            return;
        }
        let callee = function.and_then(|function| qualified_name(&self.model, function, self.source));
        self.typing_call(arguments, callee.as_deref());
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

fn is_type_name(name: &str) -> bool {
    TYPE_NAMES.contains(&name)
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
