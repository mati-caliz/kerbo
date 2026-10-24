use tree_sitter::Node;

use super::Report;
use super::ast::{
    ALIAS_FIELD, ALIASED_IMPORT, CALL, CLASS, COMMENT, FUNCTION_FIELD, IMPORT, KEYWORD_ARGUMENT, LAMBDA,
    NAME_FIELD, named, text, unparenthesized,
};
use crate::rules::PythonRule;

const ARGUMENTS_FIELD: &str = "arguments";
const SUPERCLASSES_FIELD: &str = "superclasses";
const ARGUMENT_LIST: &str = "argument_list";
const LIST_SPLAT: &str = "list_splat";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const MODULE_SEPARATOR: char = '.';
const IMPORT_FROM: &str = "import_from_statement";
const RELATIVE_IMPORT: &str = "relative_import";
const DOTTED_NAME: &str = "dotted_name";
const MODULE_NAME_FIELD: &str = "module_name";
const DEBUGGER_MODULES: [&str; 8] = [
    "pdb",
    "pudb",
    "ipdb",
    "debugpy",
    "ptvsd",
    "IPython.terminal.embed",
    "IPython.frontend.terminal.embed",
    "celery.contrib.rdb",
];
pub const DEBUGGER_CALLS: [&str; 15] = [
    "pdb.set_trace",
    "pudb.set_trace",
    "ipdb.set_trace",
    "ipdb.sset_trace",
    "IPython.terminal.embed.InteractiveShellEmbed",
    "IPython.frontend.terminal.embed.InteractiveShellEmbed",
    "celery.contrib.rdb.set_trace",
    "builtins.breakpoint",
    "debugpy.breakpoint",
    "debugpy.listen",
    "debugpy.wait_for_client",
    "ptvsd.break_into_debugger",
    "ptvsd.wait_for_attach",
    "sys.breakpointhook",
    "sys.__breakpointhook__",
];

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| match node.kind() {
            CALL => {
                let mut found = star_after_keyword(*node);
                found.extend(direct_lambda_call(*node));
                found
            }
            CLASS => empty_class_parentheses(*node).into_iter().collect(),
            IMPORT => {
                let mut found = manual_from_imports(*node, source);
                found.extend(debugger_imports(*node, None, source));
                found
            }
            IMPORT_FROM => debugger_imports(*node, imported_module(*node, source), source),
            _ => Vec::new(),
        })
        .collect()
}

// B026: `f(a=1, *rest)`, un `*` después de un argumento con nombre, que Python pasa antes que él.
fn star_after_keyword(call: Node<'_>) -> Vec<Report> {
    let arguments: Vec<Node<'_>> = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .filter(|arguments| arguments.kind() == ARGUMENT_LIST)
        .map(named)
        .unwrap_or_default();
    let Some(first_keyword) = arguments
        .iter()
        .position(|argument| matches!(argument.kind(), KEYWORD_ARGUMENT | DICTIONARY_SPLAT))
    else {
        return Vec::new();
    };
    arguments
        .iter()
        .skip(first_keyword)
        .filter(|argument| argument.kind() == LIST_SPLAT)
        .map(|argument| Report::at(PythonRule::B026, *argument, ""))
        .collect()
}

// PLC3002: `(lambda: x)()`, una lambda que se llama apenas se define.
fn direct_lambda_call(call: Node<'_>) -> Option<Report> {
    call.child_by_field_name(FUNCTION_FIELD)
        .map(unparenthesized)
        .filter(|function| function.kind() == LAMBDA)
        .map(|_| Report::at(PythonRule::PLC3002, call, ""))
}

// UP039: `class Clase():`, con paréntesis vacíos.
fn empty_class_parentheses(class: Node<'_>) -> Option<Report> {
    let arguments = class.child_by_field_name(SUPERCLASSES_FIELD)?;
    named(arguments)
        .iter()
        .all(|argument| argument.kind() == COMMENT)
        .then(|| Report::at(PythonRule::UP039, arguments, ""))
}

// PLR0402: `import a.b as b`, que se escribe `from a import b`.
fn manual_from_imports(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    named(statement)
        .into_iter()
        .filter(|alias| alias.kind() == ALIASED_IMPORT)
        .filter(|alias| {
            let module = alias
                .child_by_field_name(NAME_FIELD)
                .map(|name| text(name, source));
            let local = alias
                .child_by_field_name(ALIAS_FIELD)
                .map(|name| text(name, source));
            module
                .and_then(|module| module.rsplit_once(MODULE_SEPARATOR))
                .zip(local)
                .is_some_and(|((_, last), local)| last == local)
        })
        .map(|alias| Report::at(PythonRule::PLR0402, alias, ""))
        .collect()
}

// T100: `import pdb` o `from pdb import set_trace`. Como ruff, de un `from` relativo mira el nombre
// del módulo sin los puntos.
fn debugger_imports(statement: Node<'_>, module: Option<&str>, source: &[u8]) -> Vec<Report> {
    named(statement)
        .into_iter()
        .filter(|child| {
            statement
                .child_by_field_name(MODULE_NAME_FIELD)
                .map(|module_name| module_name.id())
                != Some(child.id())
        })
        .filter_map(|alias| match alias.kind() {
            ALIASED_IMPORT => alias.child_by_field_name(NAME_FIELD),
            DOTTED_NAME => Some(alias),
            _ => None,
        })
        .filter(|name| {
            let name = text(*name, source);
            module.map_or_else(
                || DEBUGGER_MODULES.contains(&name),
                |module| DEBUGGER_CALLS.contains(&format!("{module}.{name}").as_str()),
            )
        })
        .map(|_| Report::at(PythonRule::T100, statement, ""))
        .collect()
}

fn imported_module<'source>(statement: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let module = statement.child_by_field_name(MODULE_NAME_FIELD)?;
    let name = if module.kind() == RELATIVE_IMPORT {
        named(module)
            .into_iter()
            .find(|child| child.kind() == DOTTED_NAME)?
    } else {
        module
    };
    Some(text(name, source))
}
