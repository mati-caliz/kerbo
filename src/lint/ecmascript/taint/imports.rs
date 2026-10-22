use std::collections::HashMap;

use tree_sitter::Node;

use crate::syntax::{descendants, named_children};

const IMPORT: &str = "import_statement";
const IMPORT_CLAUSE: &str = "import_clause";
const NAMED_IMPORTS: &str = "named_imports";
const IMPORT_SPECIFIER: &str = "import_specifier";
const NAMESPACE_IMPORT: &str = "namespace_import";
const IDENTIFIER: &str = "identifier";
const DECLARATOR: &str = "variable_declarator";
const CALL: &str = "call_expression";
const MEMBER: &str = "member_expression";
const OBJECT_PATTERN: &str = "object_pattern";
const SHORTHAND: &str = "shorthand_property_identifier_pattern";
const PAIR_PATTERN: &str = "pair_pattern";
const STRING_FRAGMENT: &str = "string_fragment";
const SOURCE_FIELD: &str = "source";
const NAME_FIELD: &str = "name";
const ALIAS_FIELD: &str = "alias";
const VALUE_FIELD: &str = "value";
const KEY_FIELD: &str = "key";
const FUNCTION_FIELD: &str = "function";
const ARGUMENTS_FIELD: &str = "arguments";
const OBJECT_FIELD: &str = "object";
const PROPERTY_FIELD: &str = "property";
const REQUIRE: &str = "require";
const NODE_PREFIX: &str = "node:";
const QUALIFIER_SEPARATOR: char = '.';
const MEMBER_SEPARATOR: char = '#';
const RELATIVE: char = '.';

// Lo que nombra cada alias del archivo: `import x from 'm'` y `const x = require('m')` dejan `x` por
// `m`; `import { a as b } from 'm'` y `const { a: b } = require('m')`, `b` por `m.a`.
#[derive(Debug, Default)]
pub struct Imports {
    aliases: HashMap<String, String>,
}

fn text(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or_default().to_owned()
}

// El módulo de `'node:fs'` es `fs`: el prefijo no cambia nada.
fn module_name(literal: Node<'_>, source: &[u8]) -> Option<String> {
    let written = named_children(literal)
        .into_iter()
        .find(|part| part.kind() == STRING_FRAGMENT)
        .map(|part| text(part, source))?;
    Some(written.strip_prefix(NODE_PREFIX).unwrap_or(&written).to_owned())
}

impl Imports {
    pub fn of(root: Node<'_>, source: &[u8]) -> Self {
        let mut imports = Self::default();
        for node in descendants(root) {
            match node.kind() {
                IMPORT => imports.add_import(node, source),
                DECLARATOR => imports.add_require(node, source),
                _ => {}
            }
        }
        imports
    }

    fn add_import(&mut self, node: Node<'_>, source: &[u8]) {
        let Some(module) = node
            .child_by_field_name(SOURCE_FIELD)
            .and_then(|literal| module_name(literal, source))
        else {
            return;
        };
        let parts = named_children(node)
            .into_iter()
            .filter(|child| child.kind() == IMPORT_CLAUSE)
            .flat_map(named_children);
        for part in parts {
            self.add_clause_part(part, &module, source);
        }
    }

    // `import x`, `import * as x` o `import { a, b as c }`.
    fn add_clause_part(&mut self, part: Node<'_>, module: &str, source: &[u8]) {
        match part.kind() {
            IDENTIFIER => {
                self.aliases.insert(text(part, source), module.to_owned());
            }
            NAMESPACE_IMPORT => {
                for name in named_children(part) {
                    self.aliases.insert(text(name, source), module.to_owned());
                }
            }
            NAMED_IMPORTS => self.add_specifiers(part, module, source),
            _ => {}
        }
    }

    fn add_specifiers(&mut self, named: Node<'_>, module: &str, source: &[u8]) {
        for specifier in named_children(named)
            .into_iter()
            .filter(|child| child.kind() == IMPORT_SPECIFIER)
        {
            let Some(name) = specifier.child_by_field_name(NAME_FIELD) else {
                continue;
            };
            let alias = specifier.child_by_field_name(ALIAS_FIELD).unwrap_or(name);
            self.aliases
                .insert(text(alias, source), member_of(module, &text(name, source)));
        }
    }

    // `const x = require('m')`, `const { a, b: c } = require('m')` y `const x = require('m').y`.
    fn add_require(&mut self, declarator: Node<'_>, source: &[u8]) {
        let (Some(target), Some(value)) = (
            declarator.child_by_field_name(NAME_FIELD),
            declarator.child_by_field_name(VALUE_FIELD),
        ) else {
            return;
        };
        let Some(module) = required_module(value, source) else {
            return;
        };
        match target.kind() {
            IDENTIFIER => {
                self.aliases.insert(text(target, source), module);
            }
            OBJECT_PATTERN => {
                for part in named_children(target) {
                    let (name, alias) = match part.kind() {
                        SHORTHAND => (text(part, source), text(part, source)),
                        PAIR_PATTERN => match (
                            part.child_by_field_name(KEY_FIELD),
                            part.child_by_field_name(VALUE_FIELD),
                        ) {
                            (Some(key), Some(alias)) => (text(key, source), text(alias, source)),
                            _ => continue,
                        },
                        _ => continue,
                    };
                    self.aliases.insert(alias, member_of(&module, &name));
                }
            }
            _ => {}
        }
    }

    // El nombre completo de algo escrito con un alias del archivo.
    pub fn qualify(&self, written: &str) -> Option<String> {
        let (first, rest) = written
            .split_once(QUALIFIER_SEPARATOR)
            .map_or((written, None), |(first, rest)| (first, Some(rest)));
        let module = self.aliases.get(first)?;
        Some(rest.map_or_else(|| module.clone(), |rest| member_of(module, rest)))
    }
}

// Un miembro de un módulo: `fs.readFile`, y para un archivo del proyecto `./utils#escape`, porque el
// nombre del archivo puede tener puntos (`./config.service`).
fn member_of(module: &str, member: &str) -> String {
    if module.starts_with(RELATIVE) && !module.contains(MEMBER_SEPARATOR) {
        format!("{module}{MEMBER_SEPARATOR}{member}")
    } else {
        format!("{module}{QUALIFIER_SEPARATOR}{member}")
    }
}

// El módulo de un `require('m')`, también con una propiedad detrás (`require('m').y`).
fn required_module(value: Node<'_>, source: &[u8]) -> Option<String> {
    match value.kind() {
        CALL => {
            let function = value.child_by_field_name(FUNCTION_FIELD)?;
            if function.utf8_text(source).ok()? != REQUIRE {
                return None;
            }
            let literal = value
                .child_by_field_name(ARGUMENTS_FIELD)
                .map(named_children)
                .unwrap_or_default()
                .into_iter()
                .next()?;
            module_name(literal, source)
        }
        MEMBER => {
            let module = required_module(value.child_by_field_name(OBJECT_FIELD)?, source)?;
            let property = value.child_by_field_name(PROPERTY_FIELD)?;
            Some(member_of(&module, &text(property, source)))
        }
        _ => None,
    }
}
