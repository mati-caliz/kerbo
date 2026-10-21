mod arithmetic;
mod builtins;
mod conditions;
mod expansions;
mod quoting;
mod redirects;
mod shebang;
mod single_quotes;

use super::context::{Context, Finding, Findings};
use super::syntax::descendants;
use super::variables;

pub fn check(context: &Context<'_, '_>) -> Vec<Finding> {
    let mut findings = Findings::default();
    shebang::check(context, &mut findings);
    for node in descendants(context.root) {
        expansions::check(node, context, &mut findings);
        builtins::check(node, context, &mut findings);
        conditions::check(node, context, &mut findings);
        redirects::check(node, context, &mut findings);
        arithmetic::check(node, context, &mut findings);
        single_quotes::check(node, context, &mut findings);
    }
    quoting::check(context, &mut findings);
    variables::check(context, &mut findings);
    findings.found
}
