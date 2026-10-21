use super::super::Visitor;
use super::super::model::{Binding, BindingId, BindingKind, Model, ScopeId};
use super::super::qualified::{is_typing, qualified_name_from};
use super::{is_dummy, is_import};
use crate::lint::python::Report;
use crate::lint::python::ast::{DECORATED, named};
use crate::rules::PythonRule;

const DECORATOR: &str = "decorator";
const OVERLOAD: &str = "overload";

#[derive(Debug, Clone, Copy)]
struct Shadow {
    binding: BindingId,
    shadowed: BindingId,
    same_scope: bool,
}

#[derive(Debug, Clone, Copy)]
struct Candidate<'model, 'tree> {
    name: &'model str,
    binding: &'model Binding<'tree>,
    shadowed: &'model Binding<'tree>,
    same_scope: bool,
}

pub fn reports(visitor: &Visitor<'_>, scope: ScopeId, latest: &[(&str, BindingId)]) -> Vec<Report> {
    latest
        .iter()
        .flat_map(|(name, id)| {
            shadows(visitor, scope, *id)
                .into_iter()
                .filter_map(move |shadow| shadow_report(visitor, name, shadow))
        })
        .collect()
}

fn shadow_report(visitor: &Visitor<'_>, name: &str, shadow: Shadow) -> Option<Report> {
    let binding = visitor.model.binding(shadow.binding)?;
    let shadowed = visitor.model.binding(shadow.shadowed)?;
    if !Model::same_branch(shadowed.branch, binding.branch) {
        return None;
    }
    if binding.kind == BindingKind::LoopVariable {
        return is_import(shadowed).then(|| Report::at_point(PythonRule::F402, binding.site.at, name));
    }
    let candidate = Candidate {
        name,
        binding,
        shadowed,
        same_scope: shadow.same_scope,
    };
    is_redefinition(visitor, candidate).then(|| Report::at_point(PythonRule::F811, binding.site.at, name))
}

fn shadows(visitor: &Visitor<'_>, scope: ScopeId, latest: BindingId) -> Vec<Shadow> {
    let mut found = Vec::new();
    let mut current = latest;
    loop {
        if let Some(outer) = visitor.model.cross_shadowed.get(&current) {
            found.push(Shadow {
                binding: current,
                shadowed: *outer,
                same_scope: false,
            });
        }
        let Some(previous) = visitor.model.shadowed_in_scope(scope, current) else {
            break;
        };
        found.push(Shadow {
            binding: current,
            shadowed: previous,
            same_scope: true,
        });
        current = previous;
    }
    found
}

fn is_redefinition(visitor: &Visitor<'_>, candidate: Candidate<'_, '_>) -> bool {
    let shadowed = candidate.shadowed;
    if shadowed.is_used() || !redefines(candidate.binding, shadowed) {
        return false;
    }
    if !candidate.same_scope {
        return is_import(shadowed);
    }
    if !is_import(shadowed) && is_dummy(candidate.name) {
        return false;
    }
    !(shadowed.kind == BindingKind::Function && is_overload(visitor, shadowed))
}

fn redefines(binding: &Binding<'_>, existing: &Binding<'_>) -> bool {
    let submodules = (binding.kind == BindingKind::SubmoduleImport && existing.kind.is_import())
        || (matches!(binding.kind, BindingKind::Import | BindingKind::FromImport)
            && existing.kind == BindingKind::SubmoduleImport);
    if submodules {
        return binding.qualified == existing.qualified;
    }
    let never_redefines = matches!(
        binding.kind,
        BindingKind::Deletion
            | BindingKind::Annotation
            | BindingKind::FutureImport
            | BindingKind::Export
            | BindingKind::Global(_)
            | BindingKind::Nonlocal(_)
    );
    let assignment =
        |kind: BindingKind| matches!(kind, BindingKind::Assignment | BindingKind::NamedExpression);
    if never_redefines || (assignment(binding.kind) && assignment(existing.kind)) {
        return false;
    }
    matches!(
        existing.kind,
        BindingKind::Class
            | BindingKind::Function
            | BindingKind::Import
            | BindingKind::FromImport
            | BindingKind::Assignment
    )
}

fn is_overload(visitor: &Visitor<'_>, function: &Binding<'_>) -> bool {
    function
        .site
        .statement
        .and_then(|statement| statement.parent())
        .filter(|parent| parent.kind() == DECORATED)
        .is_some_and(|decorated| {
            named(decorated)
                .into_iter()
                .filter(|child| child.kind() == DECORATOR)
                .filter_map(|decorator| named(decorator).first().copied())
                .filter_map(|expression| {
                    qualified_name_from(&visitor.model, function.scope, expression, visitor.source)
                })
                .any(|name| is_typing(&name, OVERLOAD))
        })
}
