use tree_sitter::Node;

use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, COMMENT, EXPRESSION_STATEMENT, FUNCTION_FIELD,
    KEYWORD_ARGUMENT, NAME_FIELD, PARENTHESIZED, VALUE_FIELD, is_f_string, is_string, named, text,
    unparenthesized,
};
use crate::lint::python::declarations::DEBUGGER_CALLS;
use crate::rules::PythonRule;

const ASSERT_RAISES: &str = "assertRaises";
const PYTEST_RAISES: &str = "pytest.raises";
const MATCH_KEYWORD: &str = "match";
const PYTEST_EXCEPTION_KEYWORD: &str = "expected_exception";
const UNITTEST_EXCEPTION_KEYWORD: &str = "exception";
const FUNC_KEYWORD: &str = "func";
const BLIND_EXCEPTIONS: [&str; 2] = ["builtins.Exception", "builtins.BaseException"];
const WITH_ITEM: &str = "with_item";
const ARGUMENT_LIST: &str = "argument_list";
const LIST_SPLAT: &str = "list_splat";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const BUILTINS_PREFIX: &str = "builtins.";
// `builtins::is_exception` de ruff para Python 3.11.
const BUILTIN_EXCEPTIONS: [&str; 65] = [
    "BaseException",
    "GeneratorExit",
    "KeyboardInterrupt",
    "SystemExit",
    "Exception",
    "ArithmeticError",
    "FloatingPointError",
    "OverflowError",
    "ZeroDivisionError",
    "AssertionError",
    "AttributeError",
    "BufferError",
    "EOFError",
    "ImportError",
    "ModuleNotFoundError",
    "LookupError",
    "IndexError",
    "KeyError",
    "MemoryError",
    "NameError",
    "UnboundLocalError",
    "OSError",
    "BlockingIOError",
    "ChildProcessError",
    "ConnectionError",
    "BrokenPipeError",
    "ConnectionAbortedError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "FileExistsError",
    "FileNotFoundError",
    "InterruptedError",
    "IsADirectoryError",
    "NotADirectoryError",
    "PermissionError",
    "ProcessLookupError",
    "TimeoutError",
    "ReferenceError",
    "RuntimeError",
    "NotImplementedError",
    "RecursionError",
    "StopAsyncIteration",
    "StopIteration",
    "SyntaxError",
    "IndentationError",
    "TabError",
    "SystemError",
    "TypeError",
    "ValueError",
    "UnicodeError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeTranslateError",
    "Warning",
    "BytesWarning",
    "DeprecationWarning",
    "FutureWarning",
    "ImportWarning",
    "PendingDeprecationWarning",
    "ResourceWarning",
    "RuntimeWarning",
    "SyntaxWarning",
    "UnicodeWarning",
    "UserWarning",
    "EncodingWarning",
];
const EXCEPTION_GROUPS: [&str; 2] = ["BaseExceptionGroup", "ExceptionGroup"];

struct Arguments<'tree> {
    positional: Vec<Node<'tree>>,
    keywords: Vec<Node<'tree>>,
}

// T100 (una llamada al depurador), B017 (`assertRaises(Exception)` o `pytest.raises(Exception)`) y
// PLW0133 (una excepción construida como sentencia, sin `raise`).
pub fn suspicious_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let qualified = qualified_name(model, function, source);
    let mut reports = Vec::new();
    if qualified
        .as_deref()
        .is_some_and(|name| DEBUGGER_CALLS.contains(&name))
    {
        reports.push(Report::at(PythonRule::T100, call, ""));
    }
    let arguments = arguments(call);
    let is_blind = is_blind_assert_raises(model, source, function, &arguments);
    let has_callable =
        arguments.positional.len() >= 2 || keyword_value(&arguments, FUNC_KEYWORD, source).is_some();
    if is_blind && has_callable {
        reports.push(Report::at(PythonRule::B017, call, ""));
    }
    let is_unaliased_item = call.parent().is_some_and(|parent| parent.kind() == WITH_ITEM);
    if is_blind && is_unaliased_item {
        reports.push(Report::at(PythonRule::B017, call, ""));
    }
    let statement = std::iter::successors(call.parent(), Node::parent)
        .find(|ancestor| ancestor.kind() != PARENTHESIZED)
        .filter(|parent| parent.kind() == EXPRESSION_STATEMENT)
        .filter(|statement| {
            named(*statement)
                .iter()
                .filter(|child| child.kind() != COMMENT)
                .count()
                == 1
        });
    if let Some(statement) = statement
        && qualified.as_deref().is_some_and(is_builtin_exception)
    {
        reports.push(Report::at(PythonRule::PLW0133, statement, ""));
    }
    reports
}

fn is_builtin_exception(name: &str) -> bool {
    name.strip_prefix(BUILTINS_PREFIX)
        .is_some_and(|name| BUILTIN_EXCEPTIONS.contains(&name) || EXCEPTION_GROUPS.contains(&name))
}

// `detect_blind_exception`.
fn is_blind_assert_raises(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    arguments: &Arguments<'_>,
) -> bool {
    let is_pytest = qualified_name(model, function, source).as_deref() == Some(PYTEST_RAISES);
    let is_assert_raises = function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, source) == ASSERT_RAISES);
    if !is_assert_raises && !is_pytest {
        return false;
    }
    if is_pytest {
        let has_match = keyword_value(arguments, MATCH_KEYWORD, source).is_some();
        let has_pattern = positional(arguments, 1)
            .map(unparenthesized)
            .is_some_and(|argument| is_string(argument) && !is_f_string(argument, source));
        if has_match || has_pattern {
            return false;
        }
    }
    let keyword = if is_pytest {
        PYTEST_EXCEPTION_KEYWORD
    } else {
        UNITTEST_EXCEPTION_KEYWORD
    };
    keyword_value(arguments, keyword, source)
        .or_else(|| positional(arguments, 0))
        .and_then(|exception| qualified_name(model, unparenthesized(exception), source))
        .is_some_and(|name| BLIND_EXCEPTIONS.contains(&name.as_str()))
}

fn arguments(call: Node<'_>) -> Arguments<'_> {
    let items = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .filter(|list| list.kind() == ARGUMENT_LIST)
        .map(named)
        .unwrap_or_default();
    let (keywords, positional) = items
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .partition(|argument| matches!(argument.kind(), KEYWORD_ARGUMENT | DICTIONARY_SPLAT));
    Arguments { positional, keywords }
}

// `find_positional`: los posicionales hasta el primer `*`.
fn positional<'tree>(arguments: &Arguments<'tree>, position: usize) -> Option<Node<'tree>> {
    arguments
        .positional
        .iter()
        .take_while(|argument| argument.kind() != LIST_SPLAT)
        .nth(position)
        .copied()
}

fn keyword_value<'tree>(arguments: &Arguments<'tree>, keyword: &str, source: &[u8]) -> Option<Node<'tree>> {
    arguments
        .keywords
        .iter()
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
        .find(|argument| {
            argument
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == keyword)
        })
        .and_then(|argument| argument.child_by_field_name(VALUE_FIELD))
}
