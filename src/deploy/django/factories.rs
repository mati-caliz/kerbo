use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tree_sitter::{Node, Parser};

use super::run_python::functions;
use super::settings::Settings;
use super::truth::{Branch, taken_branch};
use crate::deploy::python_calls::Call;
use crate::language::Language;
use crate::syntax::named_children;

const IMPORT_FROM: &str = "import_from_statement";
const DOTTED_NAME: &str = "dotted_name";
const ALIASED_IMPORT: &str = "aliased_import";
const MODULE_SEPARATOR: char = '.';
const PYTHON_EXTENSION: &str = "py";
const PACKAGE_MODULE: &str = "__init__.py";
const IDENTIFIER: &str = "identifier";
const COMMENT: &str = "comment";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const STRING: &str = "string";
const RETURN: &str = "return_statement";
const IF: &str = "if_statement";
const NAMED_PARAMETERS: &[&str] = &["typed_parameter", "default_parameter", "typed_default_parameter"];
const SEARCHED_ROOTS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Factory {
    pub method: String,
    pub forwarded: Vec<Parameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub position: usize,
    pub name: String,
}

pub fn imported(
    root: Node<'_>,
    source: &[u8],
    apps_root: &Path,
    settings: &Settings,
) -> Vec<(String, Arc<Factory>)> {
    named_children(root)
        .into_iter()
        .filter(|statement| statement.kind() == IMPORT_FROM)
        .flat_map(|statement| imported_names(statement, source))
        .filter_map(|(local, module, function)| {
            Some((local, factory(apps_root, &module, &function, settings)?))
        })
        .collect()
}

fn imported_names(statement: Node<'_>, source: &[u8]) -> Vec<(String, String, String)> {
    let Some(module) = statement
        .child_by_field_name("module_name")
        .filter(|module| module.kind() == DOTTED_NAME)
        .and_then(|module| module.utf8_text(source).ok())
    else {
        return Vec::new();
    };
    let mut cursor = statement.walk();
    statement
        .children_by_field_name("name", &mut cursor)
        .filter_map(|name| {
            let (imported, local) = if name.kind() == ALIASED_IMPORT {
                (
                    name.child_by_field_name("name")?,
                    name.child_by_field_name("alias")?,
                )
            } else {
                (name, name)
            };
            Some((
                local.utf8_text(source).ok()?.to_owned(),
                module.to_owned(),
                imported.utf8_text(source).ok()?.to_owned(),
            ))
        })
        .collect()
}

fn module_file(apps_root: &Path, module: &str) -> Option<PathBuf> {
    apps_root.ancestors().take(SEARCHED_ROOTS).find_map(|root| {
        let base = module
            .split(MODULE_SEPARATOR)
            .fold(root.to_path_buf(), |path, segment| path.join(segment));
        [base.with_extension(PYTHON_EXTENSION), base.join(PACKAGE_MODULE)]
            .into_iter()
            .find(|candidate| candidate.is_file())
    })
}

type Cache = Mutex<HashMap<(PathBuf, String), Option<Arc<Factory>>>>;

fn factory(apps_root: &Path, module: &str, function: &str, settings: &Settings) -> Option<Arc<Factory>> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let file = module_file(apps_root, module)?;
    let key = (file, function.to_owned());
    let cache = CACHE.get_or_init(Cache::default);
    if let Some(known) = cache.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
        return known.clone();
    }
    let read = read_factory(&key.0, function, settings).map(Arc::new);
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, read.clone());
    read
}

fn read_factory(file: &Path, name: &str, settings: &Settings) -> Option<Factory> {
    let source = fs::read(file).ok()?;
    let mut parser = Parser::new();
    parser.set_language(&Language::Python.grammar()?).ok()?;
    let tree = parser.parse(&source, None)?;
    let function = functions(tree.root_node(), &source)
        .into_iter()
        .rev()
        .find(|(defined, _)| defined == name)
        .map(|(_, function)| function)?;
    built_factory(function, &source, settings)
}

fn built_factory(function: Node<'_>, source: &[u8], settings: &Settings) -> Option<Factory> {
    let parameters = parameter_names(function.child_by_field_name("parameters")?, source);
    let returned = returned_call(function.child_by_field_name("body")?, source, settings)?;
    let call = Call::parse(returned, source)?;
    let forwarded = call
        .arguments
        .iter()
        .map(|argument| {
            if argument.kind() != IDENTIFIER {
                return None;
            }
            let name = argument.utf8_text(source).ok()?;
            let position = parameters.iter().position(|parameter| parameter == name)?;
            Some(Parameter {
                position,
                name: name.to_owned(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Factory {
        method: call.method.to_owned(),
        forwarded,
    })
}

fn parameter_names(parameters: Node<'_>, source: &[u8]) -> Vec<String> {
    named_children(parameters)
        .into_iter()
        .filter_map(|parameter| {
            let name = if parameter.kind() == IDENTIFIER {
                parameter
            } else if NAMED_PARAMETERS.contains(&parameter.kind()) {
                parameter
                    .child_by_field_name("name")
                    .or_else(|| named_children(parameter).into_iter().next())?
            } else {
                return None;
            };
            Some(name.utf8_text(source).ok()?.to_owned())
        })
        .collect()
}

fn is_docstring(statement: Node<'_>) -> bool {
    statement.kind() == EXPRESSION_STATEMENT
        && named_children(statement)
            .into_iter()
            .all(|child| child.kind() == STRING)
}

fn returned_call<'tree>(block: Node<'tree>, source: &[u8], settings: &Settings) -> Option<Node<'tree>> {
    for statement in named_children(block) {
        if statement.kind() == COMMENT || is_docstring(statement) {
            continue;
        }
        match statement.kind() {
            RETURN => return named_children(statement).into_iter().next(),
            IF => {
                if let Branch::Taken(branch) = taken_branch(statement, source, settings)? {
                    return returned_call(branch, source, settings);
                }
            }
            _ => return None,
        }
    }
    None
}
