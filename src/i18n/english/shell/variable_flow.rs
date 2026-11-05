use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// ShellCheck checks over the variable flow: names nothing assigns and changes lost in a subshell.
pub const VARIABLE_FLOW_TEXTS: [(ShellRule, RuleText); 4] = [
    (
        ShellRule::Sc2030,
        RuleText::new(
            "Assignment that stays inside a subshell (ShellCheck SC2030)",
            "assign outside the subshell, or print the value and capture it",
            "A subshell has its own copy of the variables, so the change disappears when it ends.",
            "(x=1); echo \"$x\" prints the old value.",
        ),
    ),
    (
        ShellRule::Sc2031,
        RuleText::new(
            "Variable read after being changed in a subshell (ShellCheck SC2031)",
            "assign outside the subshell, or print the value and capture it",
            "The change made in the subshell is not visible here.",
            "cat f | while read -r x; do n=$x; done; echo \"$n\" does not see n.",
        ),
    ),
    (
        ShellRule::Sc2153,
        RuleText::new(
            "Variable that may be misspelled: nothing assigns it and a similar one exists (ShellCheck SC2153)",
            "check the spelling, or assign it",
            "A wrong letter leaves the variable empty without any error.",
            "REMOTE=x; echo \"$REMOTES\" reads an empty variable.",
        ),
    ),
    (
        ShellRule::Sc2154,
        RuleText::new(
            "Variable read but never assigned (ShellCheck SC2154)",
            "assign it, use ${x:-default}, or declare it with a directive if it comes from outside",
            "If nothing assigns it, the script runs with an empty value.",
            "echo \"$nombre\" with no nombre= before it.",
        ),
    ),
];
