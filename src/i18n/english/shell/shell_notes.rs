use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Notes ShellCheck gives while parsing, and the last default checks.
pub const SHELL_NOTES_TEXTS: [(ShellRule, RuleText); 4] = [
    (
        ShellRule::Sc1003,
        RuleText::new(
            "Backslash before the closing single quote (ShellCheck SC1003)",
            "write 'it'\\''s' to include a single quote",
            "Inside single quotes the backslash is literal: the quote closes the string.",
            "'it\\'s' becomes 'it'\\''s'.",
        ),
    ),
    (
        ShellRule::Sc1007,
        RuleText::new(
            "Space after = in an assignment (ShellCheck SC1007)",
            "remove the space, or use var='' for an empty value",
            "The variable stays empty and the next word runs as a command.",
            "a= b becomes a=b.",
        ),
    ),
    (
        ShellRule::Sc2251,
        RuleText::new(
            "! that skips set -e (ShellCheck SC2251)",
            "use && exit 1, or make sure $? is checked",
            "A negated command never stops the script under set -e.",
            "! true becomes true && exit 1.",
        ),
    ),
    (
        ShellRule::Sc2316,
        RuleText::new(
            "Declaring command used as an argument of another (ShellCheck SC2316)",
            "use a separate command, or the option of declare",
            "It declares a variable named local or readonly.",
            "local readonly a becomes local -r a.",
        ),
    ),
];
