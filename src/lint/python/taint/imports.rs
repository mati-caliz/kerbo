use std::collections::HashMap;

use tree_sitter::Node;

use crate::syntax::{descendants, named_children};

const IMPORT: &str = "import_statement";
const IMPORT_FROM: &str = "import_from_statement";
const ALIASED_IMPORT: &str = "aliased_import";
const DOTTED_NAME: &str = "dotted_name";
const NAME_FIELD: &str = "name";
const ALIAS_FIELD: &str = "alias";
const MODULE_NAME_FIELD: &str = "module_name";
const QUALIFIER_SEPARATOR: char = '.';

// Lo que nombra cada alias del archivo: `import a.b` deja `a`, `import a.b as c` deja `c` por
// `a.b`, y `from a import b as c` deja `c` por `a.b`. Los imports de adentro de una función también
// cuentan (el código de todos los días los hace para no cargar un módulo de más).
#[derive(Debug, Default)]
pub struct Imports {
    aliases: HashMap<String, String>,
}

impl Imports {
    pub fn of(root: Node<'_>, source: &[u8]) -> Self {
        let mut imports = Self::default();
        let text = |node: Node<'_>| node.utf8_text(source).unwrap_or_default().to_owned();
        for node in descendants(root) {
            match node.kind() {
                IMPORT => imports.add_imports(node, &text),
                IMPORT_FROM => imports.add_from_import(node, &text),
                _ => {}
            }
        }
        imports
    }

    fn add_imports(&mut self, node: Node<'_>, text: &impl Fn(Node<'_>) -> String) {
        for imported in named_children(node) {
            self.add_import(imported, text);
        }
    }

    // `from modulo import a, b as c`: los nombres que vienen después del módulo.
    fn add_from_import(&mut self, node: Node<'_>, text: &impl Fn(Node<'_>) -> String) {
        let Some(module) = node.child_by_field_name(MODULE_NAME_FIELD) else {
            return;
        };
        let written = text(module);
        for imported in named_children(node)
            .into_iter()
            .filter(|child| child.start_byte() > module.end_byte())
        {
            self.add_from(&written, imported, text);
        }
    }

    fn add_import(&mut self, imported: Node<'_>, text: &impl Fn(Node<'_>) -> String) {
        match imported.kind() {
            DOTTED_NAME => {
                let written = text(imported);
                let first = written
                    .split(QUALIFIER_SEPARATOR)
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                self.aliases.insert(first.clone(), first);
            }
            ALIASED_IMPORT => {
                if let (Some(name), Some(alias)) = (
                    imported.child_by_field_name(NAME_FIELD),
                    imported.child_by_field_name(ALIAS_FIELD),
                ) {
                    self.aliases.insert(text(alias), text(name));
                }
            }
            _ => {}
        }
    }

    fn add_from(&mut self, module: &str, imported: Node<'_>, text: &impl Fn(Node<'_>) -> String) {
        let (name, alias) = match imported.kind() {
            DOTTED_NAME => (text(imported), text(imported)),
            ALIASED_IMPORT => match (
                imported.child_by_field_name(NAME_FIELD),
                imported.child_by_field_name(ALIAS_FIELD),
            ) {
                (Some(name), Some(alias)) => (text(name), text(alias)),
                _ => return,
            },
            _ => return,
        };
        let qualified = if module.ends_with(QUALIFIER_SEPARATOR) {
            format!("{module}{name}")
        } else {
            format!("{module}{QUALIFIER_SEPARATOR}{name}")
        };
        self.aliases.insert(alias, qualified);
    }

    // Si el archivo importa algo de ese paquete (`flask`, `fastapi`).
    pub fn uses(&self, package: &str) -> bool {
        self.aliases.values().any(|module| {
            module == package
                || module
                    .strip_prefix(package)
                    .is_some_and(|rest| rest.starts_with(QUALIFIER_SEPARATOR))
        })
    }

    // El nombre completo de algo escrito con un alias del archivo (`utils.f` con `from helpers import
    // utils` es `helpers.utils.f`); sin alias, nada.
    pub fn qualify(&self, written: &str) -> Option<String> {
        let (first, rest) = written
            .split_once(QUALIFIER_SEPARATOR)
            .map_or((written, None), |(first, rest)| (first, Some(rest)));
        let module = self.aliases.get(first)?;
        Some(rest.map_or_else(
            || module.clone(),
            |rest| format!("{module}{QUALIFIER_SEPARATOR}{rest}"),
        ))
    }
}
