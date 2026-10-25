mod assert_bindings;
mod redefinitions;
mod removal;
mod type_checking_imports;
mod unused_variables;

use tree_sitter::{Node, Point};

use super::Visitor;
use super::model::{
    Binding, BindingId, BindingKind, GLOBAL, MODULE_SCOPE, NAME_ERROR, NONLOCAL, ScopeId, ScopeKind, UNPACKED,
};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::builtins::is_builtin;
use crate::rules::PythonRule;
use unused_variables::unused_variable_fix;

const PATH_GLOBAL: &str = "__path__";
const TRACEBACK_NAMES: [&str; 4] = [
    "__tracebackhide__",
    "__traceback_info__",
    "__traceback_supplement__",
    "__debuggerskip__",
];
const UNUSED_KINDS: [BindingKind; 3] = [
    BindingKind::Assignment,
    BindingKind::NamedExpression,
    BindingKind::WithItem,
];

pub fn reports(visitor: &Visitor<'_>) -> Vec<Report> {
    let mut reports = undefined_names(visitor);
    for scope in 0..visitor.model.scopes.len() {
        reports.extend(scope_reports(visitor, scope));
    }
    reports
}

fn undefined_names(visitor: &Visitor<'_>) -> Vec<Report> {
    visitor
        .model
        .unresolved
        .iter()
        .filter_map(|unresolved| {
            if unresolved.wildcard {
                return Some(Report::at_point(
                    PythonRule::F405,
                    unresolved.at,
                    &unresolved.name,
                ));
            }
            let silenced = unresolved.handled & NAME_ERROR != 0
                || (visitor.file.package_init && unresolved.name == PATH_GLOBAL);
            (!silenced).then(|| Report::at_point(PythonRule::F821, unresolved.at, &unresolved.name))
        })
        .collect()
}

fn scope_reports(visitor: &Visitor<'_>, scope: ScopeId) -> Vec<Report> {
    let model = &visitor.model;
    let kind = model.scope_of(scope).kind;
    let mut latest: Vec<(&str, BindingId)> = model
        .scope_of(scope)
        .names
        .iter()
        .map(|(name, id)| (name.as_str(), *id))
        .collect();
    latest.sort_unstable_by_key(|(_, id)| *id);
    let mut reports = redefinitions::reports(visitor, scope, &latest);
    reports.extend(super::private_types::unused_private_types(visitor, &latest));
    reports.extend(assert_bindings::assignments_in_assert(visitor, &latest));
    if matches!(kind, ScopeKind::Module | ScopeKind::Function) {
        reports.extend(removal::unused_imports(visitor, &latest));
        if !visitor.file.stub {
            reports.extend(type_checking_imports::runtime_imports(visitor, &latest));
        }
    }
    if kind == ScopeKind::Function {
        reports.extend(unused_locals(visitor, scope, &latest));
        reports.extend(undefined_locals(visitor, scope, &latest));
    }
    reports
}

fn unused_locals(visitor: &Visitor<'_>, scope: ScopeId, latest: &[(&str, BindingId)]) -> Vec<Report> {
    let model = &visitor.model;
    let uses_locals = model.scope_of(scope).uses_locals;
    latest
        .iter()
        .filter_map(|(name, id)| model.binding(*id).map(|binding| (*name, binding)))
        .filter(|(name, binding)| !binding.is_used() && !is_dummy(name))
        .filter_map(|(name, binding)| {
            if binding.kind == BindingKind::Annotation {
                return Some(Report::at_point(PythonRule::F842, binding.site.at, name));
            }
            let reported = UNUSED_KINDS.contains(&binding.kind)
                && !uses_locals
                && !binding.has(GLOBAL | NONLOCAL)
                && !TRACEBACK_NAMES.contains(&name);
            if !reported {
                return None;
            }
            Some(if binding.has(UNPACKED) {
                Report::at_point(PythonRule::RUF059, binding.site.at, name)
            } else {
                Report::at_point(PythonRule::F841, binding.site.at, name).with_fix(unused_variable_fix(
                    model,
                    visitor.source,
                    binding,
                ))
            })
        })
        .collect()
}

fn undefined_locals(visitor: &Visitor<'_>, scope: ScopeId, latest: &[(&str, BindingId)]) -> Vec<Report> {
    let model = &visitor.model;
    latest
        .iter()
        .filter_map(|(name, id)| {
            let shadowed = model.binding(*model.cross_shadowed.get(id)?)?;
            let reference = shadowed
                .references
                .iter()
                .find(|reference| reference.scope == scope && !reference.forward)?;
            Some(Report::at_point(PythonRule::F823, reference.at, name))
        })
        .collect()
}

pub fn is_dummy(name: &str) -> bool {
    name.starts_with('_') && (name.chars().all(|letter| letter == '_') || !name.ends_with('_'))
}

pub fn is_import(binding: &Binding<'_>) -> bool {
    binding.kind.is_import() || binding.kind == BindingKind::FutureImport
}

fn undefined_export(visitor: &Visitor<'_>, name: &str, at: Point) -> Option<Report> {
    if is_builtin(name) {
        return None;
    }
    if visitor.model.scope_of(MODULE_SCOPE).star_import {
        return Some(Report::at_point(PythonRule::F405, at, name));
    }
    (!visitor.file.package_init).then(|| Report::at_point(PythonRule::F822, at, name))
}

pub trait Exports<'tree> {
    fn visit_exports(&mut self);
    fn unused_exception(&mut self, name: Node<'tree>, id: BindingId, pattern: Option<Node<'tree>>);
}

impl<'tree> Exports<'tree> for Visitor<'tree> {
    fn visit_exports(&mut self) {
        self.model.scope = MODULE_SCOPE;
        let exports = std::mem::take(&mut self.exports);
        for (name, at) in exports {
            let found = self.model.scope_of(MODULE_SCOPE).names.get(&name).copied();
            if let Some(id) = found {
                self.model.reference(id, at);
            } else if let Some(report) = undefined_export(self, &name, at) {
                self.reports.push(report);
            }
        }
    }

    fn unused_exception(&mut self, name: Node<'tree>, id: BindingId, pattern: Option<Node<'tree>>) {
        if self.model.binding(id).is_none_or(Binding::is_used) {
            return;
        }
        let fix = pattern.and_then(|pattern| {
            let exception = crate::lint::python::ast::named(pattern).first().copied()?;
            let edit = TextEdit {
                start: TextPoint::from_point(exception.end_position()),
                end: TextPoint::from_point(pattern.end_position()),
                replacement: String::new(),
            };
            Some(Fix::from_edits(self.source, vec![edit]))
        });
        let subject = self.text(name);
        self.reports
            .push(Report::at(PythonRule::F841, name, subject).with_fix(fix));
    }
}
