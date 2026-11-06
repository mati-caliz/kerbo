use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// ShellCheck's Checks/Commands.hs and Checks/ShellSupport.hs checks on the details of a command: the printf format, the getopts case, declarations and what each shell supports.
pub const COMMAND_DETAILS_TEXTS: [(ShellRule, RuleText); 19] = [
    (
        ShellRule::Sc2025,
        RuleText::new(
            "Escape sequence in PS1 outside \\[..\\] (ShellCheck SC2025)",
            "enclose the sequences in \\[..\\]",
            "Otherwise the prompt's length is miscounted and long lines wrap badly.",
            "PS1='\\e[0m$ ' becomes PS1='\\[\\e[0m\\]$ '.",
        ),
    ),
    (
        ShellRule::Sc2051,
        RuleText::new(
            "Variable in a brace range (ShellCheck SC2051)",
            "use seq or a for ((..)) loop",
            "Bash expands braces before variables, so the range stays as text.",
            "{1..$n} becomes $(seq 1 \"$n\").",
        ),
    ),
    (
        ShellRule::Sc2175,
        RuleText::new(
            "Brace range with a variable passed to eval (ShellCheck SC2175)",
            "quote it, since it has to reach eval literally",
            "Unquoted, it is read twice.",
            "eval echo {1..$n} becomes eval echo \"{1..$n}\".",
        ),
    ),
    (
        ShellRule::Sc2176,
        RuleText::new(
            "time with a pipeline in sh (ShellCheck SC2176)",
            "time a single stage, or use bash -c",
            "time is not defined for pipelines there.",
            "time a | b becomes bash -c 'time a | b'.",
        ),
    ),
    (
        ShellRule::Sc2177,
        RuleText::new(
            "time with a compound command in sh (ShellCheck SC2177)",
            "use time sh -c",
            "time is not defined for compound commands there.",
            "time { a; } becomes time sh -c 'a'.",
        ),
    ),
    (
        ShellRule::Sc2180,
        RuleText::new(
            "Multidimensional array in bash (ShellCheck SC2180)",
            "use a 1D or an associative array",
            "Bash does not have them.",
            "a=([1][2]=3) becomes a=([1,2]=3).",
        ),
    ),
    (
        ShellRule::Sc2182,
        RuleText::new(
            "printf format with no variables but with arguments (ShellCheck SC2182)",
            "add the format, or remove the arguments",
            "The arguments are ignored and the text is repeated.",
            "printf foo bar becomes printf '%s %s' foo bar.",
        ),
    ),
    (
        ShellRule::Sc2183,
        RuleText::new(
            "printf format with a different number of variables than arguments (ShellCheck SC2183)",
            "match the format and the arguments",
            "Missing values become empty, and extra ones repeat the format.",
            "printf '%s %s' a becomes printf '%s %s' a b.",
        ),
    ),
    (
        ShellRule::Sc2213,
        RuleText::new(
            "getopts flag that the case does not handle (ShellCheck SC2213)",
            "add the flag to the case, or remove it from getopts",
            "The flag is accepted and nothing happens.",
            "getopts 'ab' with only a) needs b).",
        ),
    ),
    (
        ShellRule::Sc2214,
        RuleText::new(
            "case branch for a flag getopts does not declare (ShellCheck SC2214)",
            "add the flag to getopts, or remove the branch",
            "getopts never returns that flag.",
            "getopts 'a' with b) needs getopts 'ab'.",
        ),
    ),
    (
        ShellRule::Sc2220,
        RuleText::new(
            "getopts case without a branch for invalid flags (ShellCheck SC2220)",
            "add a *) branch",
            "An invalid flag passes silently.",
            "case $o in a) x;; esac needs *) usage;;.",
        ),
    ),
    (
        ShellRule::Sc2290,
        RuleText::new(
            "Spaces around = in a declaration (ShellCheck SC2290)",
            "remove the spaces",
            "The value is a separate argument: the variable does not get it.",
            "export a =b becomes export a=b.",
        ),
    ),
    (
        ShellRule::Sc2291,
        RuleText::new(
            "Repeated spaces between echo arguments (ShellCheck SC2291)",
            "quote them",
            "Unquoted, they collapse into one.",
            "echo a    b becomes echo \"a    b\".",
        ),
    ),
    (
        ShellRule::Sc2302,
        RuleText::new(
            "for over values whose variable is used as an index (ShellCheck SC2302)",
            "to loop over keys, use \"${!array[@]}\"",
            "The variable holds values, not positions.",
            "for i in \"${a[@]}\" becomes for i in \"${!a[@]}\".",
        ),
    ),
    (
        ShellRule::Sc2303,
        RuleText::new(
            "Array value used as a key (ShellCheck SC2303)",
            "use the value directly, or loop over the keys",
            "The variable holds values, not positions.",
            "${a[$i]} becomes $i.",
        ),
    ),
    (
        ShellRule::Sc2318,
        RuleText::new(
            "Assignment used again in the same declaration (ShellCheck SC2318)",
            "use two declarations",
            "The value has not taken effect yet.",
            "declare a=1 b=$a becomes declare a=1; declare b=$a.",
        ),
    ),
    (
        ShellRule::Sc2325,
        RuleText::new(
            "Several ! in front of a pipeline in sh (ShellCheck SC2325)",
            "use one at most",
            "It is a bash and ksh extension.",
            "! ! a becomes a.",
        ),
    ),
    (
        ShellRule::Sc2326,
        RuleText::new(
            "! in the middle of a pipeline (ShellCheck SC2326)",
            "use a group: cmd | { ! cmd; }",
            "It is not allowed there.",
            "a | ! b becomes a | { ! b; }.",
        ),
    ),
    (
        ShellRule::Sc2332,
        RuleText::new(
            "[ ! -a x ] or [ ! -o x ] in bash (ShellCheck SC2332)",
            "use -e, or ! [ -o opt ]",
            "With !, -a and -o read as and and or: the result is always true.",
            "[ ! -a f ] becomes [ ! -e f ].",
        ),
    ),
];
