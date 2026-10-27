use super::paths::{DerivedConstructor, SuperProblem};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const CONSTRUCTOR_PROBLEMS: [SuperProblem; 4] = [
    SuperProblem::MissingSome,
    SuperProblem::MissingAll,
    SuperProblem::Duplicate,
    SuperProblem::NotConstructor,
];

// `constructor-super`: el constructor de una clase con `extends` que no llama a `super()` en todos
// los caminos, que lo llama dos veces o que extiende algo que no es un constructor.
pub fn constructor_supers(context: &Context<'_, '_>) -> Vec<LintFinding> {
    findings(context, EslintRule::ConstructorSuper, &CONSTRUCTOR_PROBLEMS)
}

// `no-this-before-super`: `this` o `super.x` antes de llamar a `super()` en algún camino.
pub fn this_before_supers(context: &Context<'_, '_>) -> Vec<LintFinding> {
    findings(
        context,
        EslintRule::NoThisBeforeSuper,
        &[SuperProblem::BeforeSuper],
    )
}

fn findings(context: &Context<'_, '_>, rule: EslintRule, problems: &[SuperProblem]) -> Vec<LintFinding> {
    context
        .paths()
        .paths
        .iter()
        .filter_map(|path| path.derived.as_ref())
        .flat_map(DerivedConstructor::reports)
        .filter(|found| problems.contains(&found.problem))
        .map(|found| report(rule, found.node, ""))
        .collect()
}
