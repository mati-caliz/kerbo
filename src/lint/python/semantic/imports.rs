use tree_sitter::Node;

use super::Visitor;
use super::model::{BindingKind, EXPLICIT_EXPORT, Site};
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ALIAS_FIELD, ALIASED_IMPORT, DOTTED_NAME, MODULE_NAME_FIELD, NAME_FIELD, WILDCARD_IMPORT, children,
    fields,
};
use crate::lint::python::semantic::boundary::Boundary;
use crate::rules::PythonRule;

const SEPARATOR: char = '.';
const WILDCARD: &str = "*";
const MEMBER_SEPARATOR: &str = ", ";
const BUILTIN_MODULES: [&str; 2] = ["builtins", "six.moves.builtins"];
const BUILTIN_MEMBERS: [&str; 24] = [
    "*",
    "ascii",
    "bytes",
    "chr",
    "dict",
    "filter",
    "hex",
    "input",
    "int",
    "isinstance",
    "list",
    "map",
    "max",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "pow",
    "range",
    "round",
    "str",
    "super",
    "zip",
];
const SIX_MEMBERS: [&str; 2] = ["callable", "next"];
const SIX_MOVES_MEMBERS: [&str; 5] = ["filter", "input", "map", "range", "zip"];
const IO_MODULE: &str = "io";
const IO_OPEN: &str = "open";
const SIX: &str = "six";
const SIX_MOVES: &str = "six.moves";

pub struct Imported<'tree> {
    name: &'tree str,
    qualified: String,
    node: Node<'tree>,
    flags: u8,
}

pub trait Imports<'tree> {
    fn import(&mut self, statement: Node<'tree>);
    fn import_from(&mut self, statement: Node<'tree>);
    fn future_import(&mut self, statement: Node<'tree>);
    fn aliased(&self, aliased: Node<'tree>, module: Option<&str>) -> Option<Imported<'tree>>;
    fn bind_import(&mut self, statement: Node<'tree>, imported: Imported<'tree>, kind: BindingKind);
    fn useless_import_alias(&mut self, name: Node<'tree>);
    fn unnecessary_builtin_imports(&mut self, statement: Node<'tree>, module: &str);
}

impl<'tree> Imports<'tree> for Visitor<'tree> {
    fn import(&mut self, statement: Node<'tree>) {
        self.import_out_of_place(statement);
        for name in fields(statement, NAME_FIELD) {
            self.useless_import_alias(name);
            if name.kind() == DOTTED_NAME {
                let (imported, kind) = dotted_import(name, self.text(name));
                self.bind_import(statement, imported, kind);
            } else if let Some(imported) = self.aliased(name, None) {
                self.bind_import(statement, imported, BindingKind::Import);
            }
        }
    }

    fn import_from(&mut self, statement: Node<'tree>) {
        self.import_out_of_place(statement);
        let module = statement
            .child_by_field_name(MODULE_NAME_FIELD)
            .map(|module| self.text(module))
            .unwrap_or_default();
        if children(statement)
            .iter()
            .any(|child| child.kind() == WILDCARD_IMPORT)
        {
            if is_builtin_import(module, WILDCARD) {
                self.reports
                    .push(Report::at(PythonRule::UP029, statement, WILDCARD));
            }
            if let Some(scope) = self.model.current_mut() {
                scope.star_import = true;
            }
            return;
        }
        for name in fields(statement, NAME_FIELD) {
            self.useless_import_alias(name);
            let imported = if name.kind() == DOTTED_NAME {
                let member = self.text(name);
                Some(Imported {
                    name: member,
                    qualified: member_of(module, member),
                    node: name,
                    flags: 0,
                })
            } else {
                self.aliased(name, Some(module))
            };
            if let Some(imported) = imported {
                self.bind_import(statement, imported, BindingKind::FromImport);
            }
        }
        self.unnecessary_builtin_imports(statement, module);
    }

    fn future_import(&mut self, statement: Node<'tree>) {
        self.import_out_of_place(statement);
        for name in fields(statement, NAME_FIELD) {
            let imported = if name.kind() == ALIASED_IMPORT {
                self.aliased(name, None)
            } else {
                Some(Imported {
                    name: self.text(name),
                    qualified: self.text(name).to_owned(),
                    node: name,
                    flags: 0,
                })
            };
            if let Some(imported) = imported {
                self.bind_import(statement, imported, BindingKind::FutureImport);
            }
        }
    }

    fn aliased(&self, aliased: Node<'tree>, module: Option<&str>) -> Option<Imported<'tree>> {
        if aliased.kind() != ALIASED_IMPORT {
            return None;
        }
        let original = self.text(aliased.child_by_field_name(NAME_FIELD)?);
        let alias = aliased.child_by_field_name(ALIAS_FIELD)?;
        let name = self.text(alias);
        Some(Imported {
            name,
            qualified: module.map_or_else(|| original.to_owned(), |module| member_of(module, original)),
            node: alias,
            flags: if name == original { EXPLICIT_EXPORT } else { 0 },
        })
    }

    fn bind_import(&mut self, statement: Node<'tree>, imported: Imported<'tree>, kind: BindingKind) {
        let site = Site {
            at: imported.node.start_position(),
            node: Some(imported.node),
            statement: Some(statement),
        };
        let id = self.model.add_binding(imported.name, kind, site);
        self.model.flag(id, imported.flags);
        if let Some(binding) = self.model.bindings.get_mut(id) {
            binding.qualified = imported.qualified;
        }
    }

    // PLC0414: `import x as x`, fuera de stubs y de `__init__.py`, donde sirve para reexportar.
    // UP029: `from builtins import zip` (o de `six`, `io`), que ya están sin importar. Como ruff, sólo
    // los nombres sin alias que no pisan otro binding del mismo alcance.
    fn unnecessary_builtin_imports(&mut self, statement: Node<'tree>, module: &str) {
        let scope = self.model.scope;
        let unnecessary: Vec<&str> = fields(statement, NAME_FIELD)
            .into_iter()
            .filter(|name| name.kind() == DOTTED_NAME)
            .map(|name| self.text(name))
            .filter(|member| is_builtin_import(module, member))
            .filter(|member| {
                self.model
                    .scope_of(scope)
                    .names
                    .get(*member)
                    .is_some_and(|id| self.model.shadowed_in_scope(scope, *id).is_none())
            })
            .collect();
        if !unnecessary.is_empty() {
            self.reports.push(Report::at(
                PythonRule::UP029,
                statement,
                &unnecessary.join(MEMBER_SEPARATOR),
            ));
        }
    }

    fn useless_import_alias(&mut self, name: Node<'tree>) {
        if name.kind() != ALIASED_IMPORT || self.file.stub || self.file.package_init {
            return;
        }
        let original = name
            .child_by_field_name(NAME_FIELD)
            .map(|original| self.text(original));
        let alias = name
            .child_by_field_name(ALIAS_FIELD)
            .map(|alias| self.text(alias));
        if original.is_some() && original == alias {
            let fix = alias.map(|alias| Fix::replace(name, alias, self.source).marked_unsafe());
            self.reports
                .push(Report::at(PythonRule::PLC0414, name, "").with_fix(fix));
        }
    }
}

fn member_of(module: &str, member: &str) -> String {
    if module.ends_with(SEPARATOR) {
        format!("{module}{member}")
    } else {
        format!("{module}{SEPARATOR}{member}")
    }
}

fn dotted_import<'tree>(name: Node<'tree>, full: &'tree str) -> (Imported<'tree>, BindingKind) {
    let (bound, kind) = match full.split_once(SEPARATOR) {
        Some((module, _)) => (module, BindingKind::SubmoduleImport),
        None => (full, BindingKind::Import),
    };
    let imported = Imported {
        name: bound,
        qualified: full.to_owned(),
        node: name,
        flags: 0,
    };
    (imported, kind)
}

fn is_builtin_import(module: &str, member: &str) -> bool {
    if BUILTIN_MODULES.contains(&module) {
        BUILTIN_MEMBERS.contains(&member)
    } else {
        match module {
            IO_MODULE => member == IO_OPEN,
            SIX => SIX_MEMBERS.contains(&member),
            SIX_MOVES => SIX_MOVES_MEMBERS.contains(&member),
            _ => false,
        }
    }
}
