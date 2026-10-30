mod analytics;
mod arithmetic;
mod bashisms;
mod builtins;
mod command_checks;
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
    let bashisms = bashisms::Bashisms::new(context);
    for node in descendants(context.root) {
        if let Some(bashisms) = &bashisms {
            bashisms.check(node, context, &mut findings);
        }
        expansions::check(node, context, &mut findings);
        builtins::check(node, context, &mut findings);
        command_checks::check(node, context, &mut findings);
        analytics::check(node, context, &mut findings);
        conditions::check(node, context, &mut findings);
        redirects::check(node, context, &mut findings);
        arithmetic::check(node, context, &mut findings);
        single_quotes::check(node, context, &mut findings);
    }
    analytics::check_script(context, &mut findings);
    quoting::check(context, &mut findings);
    variables::check(context, &mut findings);
    findings.found
}
