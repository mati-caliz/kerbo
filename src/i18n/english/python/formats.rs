use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

const RUNTIME_WHY: &str = "The line fails only when it runs, with TypeError, KeyError or ValueError.";
const PERCENT_EXAMPLE: &str = "\"%s and %s\" % (a,) becomes \"%s and %s\" % (a, b), or better an f-string.";
const FORMAT_EXAMPLE: &str =
    "\"{} and {}\".format(a) becomes \"{} and {}\".format(a, b), or better an f-string.";

pub const FORMAT_TEXTS: [(PythonRule, RuleText); 14] = [
    (
        PythonRule::F501,
        RuleText::new(
            "Invalid % format string",
            "fix the specifier or escape the % as %%",
            RUNTIME_WHY,
            "\"100%\" % value becomes \"100%%\" % value.",
        ),
    ),
    (
        PythonRule::F502,
        RuleText::new(
            "The % format expects a mapping and gets a sequence",
            "pass a dictionary with the keys the format uses",
            RUNTIME_WHY,
            "\"%(name)s\" % (name,) becomes \"%(name)s\" % {\"name\": name}.",
        ),
    ),
    (
        PythonRule::F503,
        RuleText::new(
            "The % format expects a sequence and gets a mapping",
            "pass a tuple with one value per specifier",
            RUNTIME_WHY,
            PERCENT_EXAMPLE,
        ),
    ),
    (
        PythonRule::F504,
        RuleText::new(
            "Dictionary keys the % format does not use",
            "remove the extra keys or use them in the format",
            "A key nobody reads is usually a typo in the format.",
            "\"%(a)s\" % {\"a\": 1, \"b\": 2} becomes \"%(a)s\" % {\"a\": 1}.",
        ),
    ),
    (
        PythonRule::F505,
        RuleText::new(
            "The dictionary lacks keys the % format asks for",
            "add the missing keys",
            RUNTIME_WHY,
            "\"%(a)s %(b)s\" % {\"a\": 1} becomes \"%(a)s %(b)s\" % {\"a\": 1, \"b\": 2}.",
        ),
    ),
    (
        PythonRule::F506,
        RuleText::new(
            "The % format mixes named and positional specifiers",
            "use all named or all positional",
            RUNTIME_WHY,
            "\"%s %(a)s\" becomes \"%(b)s %(a)s\".",
        ),
    ),
    (
        PythonRule::F507,
        RuleText::new(
            "The % format gets a different number of values than it asks for",
            "pass one value per specifier",
            RUNTIME_WHY,
            PERCENT_EXAMPLE,
        ),
    ),
    (
        PythonRule::F508,
        RuleText::new(
            "The * in a % format needs a sequence",
            "pass a tuple with the width or precision before the value",
            RUNTIME_WHY,
            "\"%*d\" % {\"a\": 1} becomes \"%*d\" % (5, 1).",
        ),
    ),
    (
        PythonRule::F509,
        RuleText::new(
            "Conversion character the % format does not support",
            "use a valid conversion (s, r, d, f...)",
            RUNTIME_WHY,
            "\"%y\" % value becomes \"%s\" % value.",
        ),
    ),
    (
        PythonRule::F521,
        RuleText::new(
            "Invalid .format string",
            "fix the braces or escape them as {{ and }}",
            RUNTIME_WHY,
            "\"{\".format(a) becomes \"{{\".format().",
        ),
    ),
    (
        PythonRule::F522,
        RuleText::new(
            "Named arguments .format does not use",
            "remove the extra arguments or use them in the format",
            "An argument nobody reads is usually a typo in the format.",
            "\"{a}\".format(a=1, b=2) becomes \"{a}\".format(a=1).",
        ),
    ),
    (
        PythonRule::F523,
        RuleText::new(
            "Positional arguments .format does not use",
            "remove the extra arguments or use them in the format",
            "An argument nobody reads is usually a mistake in the format.",
            "\"{0}\".format(1, 2) becomes \"{0}\".format(1).",
        ),
    ),
    (
        PythonRule::F524,
        RuleText::new(
            ".format lacks arguments the format asks for",
            "add the missing arguments",
            RUNTIME_WHY,
            FORMAT_EXAMPLE,
        ),
    ),
    (
        PythonRule::F525,
        RuleText::new(
            ".format mixes numbered and automatic fields",
            "number every field or none",
            RUNTIME_WHY,
            "\"{0} {}\" becomes \"{0} {1}\".",
        ),
    ),
];
