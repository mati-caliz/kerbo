use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// ShellCheck Analytics.hs checks on the operands of [ ] and [[ ]].
pub const TEST_OPERANDS_TEXTS: [(ShellRule, RuleText); 22] = [
    (
        ShellRule::Sc2050,
        RuleText::new(
            "Comparison between two constants (ShellCheck SC2050)",
            "add the $ that is missing on the variable",
            "The result is always the same, so the condition decides nothing.",
            "[ n -le 4 ] becomes [ \"$n\" -le 4 ].",
        ),
    ),
    (
        ShellRule::Sc2076,
        RuleText::new(
            "Quoted right side of =~ (ShellCheck SC2076)",
            "remove the quotes so it matches as a regex",
            "With quotes the text is compared literally, not as an expression.",
            "[[ $a =~ \"x.*\" ]] becomes [[ $a =~ x.* ]].",
        ),
    ),
    (
        ShellRule::Sc2077,
        RuleText::new(
            "Comparison without spaces around the operator (ShellCheck SC2077)",
            "put spaces around the operator",
            "Without spaces it is a single word, and a non-empty word is always true.",
            "[ $a=3 ] becomes [ \"$a\" = 3 ].",
        ),
    ),
    (
        ShellRule::Sc2078,
        RuleText::new(
            "Condition with a constant word (ShellCheck SC2078)",
            "add the $ that is missing, or the command without brackets",
            "A non-empty word is always true.",
            "[ cmd ] becomes cmd.",
        ),
    ),
    (
        ShellRule::Sc2143,
        RuleText::new(
            "Comparing the output of grep instead of using -q (ShellCheck SC2143)",
            "use grep -q as the condition",
            "grep -q stops at the first match and does not build a string.",
            "[ -n \"$(grep x f)\" ] becomes grep -q x f.",
        ),
    ),
    (
        ShellRule::Sc2144,
        RuleText::new(
            "File test on a glob (ShellCheck SC2144)",
            "use a for loop",
            "With several matches the test receives too many arguments.",
            "[ -e *.txt ] becomes for f in *.txt; do [ -e \"$f\" ]; done.",
        ),
    ),
    (
        ShellRule::Sc2157,
        RuleText::new(
            "-n or -z on a word that always has text (ShellCheck SC2157)",
            "remove the literal text from the operand, or the check",
            "With literal text in the word, the result is always the same.",
            "[ -n \"$a \" ] becomes [ -n \"$a\" ].",
        ),
    ),
    (
        ShellRule::Sc2158,
        RuleText::new(
            "[ false ] is true (ShellCheck SC2158)",
            "remove the brackets",
            "Any non-empty word inside [ ] is true, even the word false.",
            "[ false ] becomes false.",
        ),
    ),
    (
        ShellRule::Sc2159,
        RuleText::new(
            "[ 0 ] is true (ShellCheck SC2159)",
            "use false",
            "Any non-empty word inside [ ] is true, even a zero.",
            "[ 0 ] becomes false.",
        ),
    ),
    (
        ShellRule::Sc2160,
        RuleText::new(
            "[ true ] instead of true (ShellCheck SC2160)",
            "use true without brackets",
            "The brackets add nothing to the command.",
            "[ true ] becomes true.",
        ),
    ),
    (
        ShellRule::Sc2161,
        RuleText::new(
            "[ 1 ] instead of true (ShellCheck SC2161)",
            "use true",
            "It reads as a number, but it is just a non-empty word.",
            "[ 1 ] becomes true.",
        ),
    ),
    (
        ShellRule::Sc2193,
        RuleText::new(
            "Comparison that can never be equal (ShellCheck SC2193)",
            "check the text of both sides",
            "The fixed parts of the two sides do not match for any value.",
            "[[ x$a == y* ]] never matches.",
        ),
    ),
    (
        ShellRule::Sc2198,
        RuleText::new(
            "Array as an operand of [ ] (ShellCheck SC2198)",
            "use a loop, or concatenate with * instead of @",
            "Each element becomes a separate argument of the test.",
            "[ \"$@\" = x ] compares only with several arguments.",
        ),
    ),
    (
        ShellRule::Sc2199,
        RuleText::new(
            "Array in [[ ]], which concatenates it (ShellCheck SC2199)",
            "use a loop, or an explicit * instead of @",
            "The elements get joined into one word and the comparison looks at all of them at once.",
            "[[ $@ = x ]] compares the whole list.",
        ),
    ),
    (
        ShellRule::Sc2200,
        RuleText::new(
            "Brace expansion as an operand of [ ] (ShellCheck SC2200)",
            "use a loop",
            "Each expansion becomes a separate argument of the test.",
            "[ $a = {x,y} ] gets too many arguments.",
        ),
    ),
    (
        ShellRule::Sc2201,
        RuleText::new(
            "Brace expansion in [[ ]], where it does not happen (ShellCheck SC2201)",
            "use a loop",
            "The text is compared with the braces included.",
            "[[ $a == {x,y} ]] never expands.",
        ),
    ),
    (
        ShellRule::Sc2202,
        RuleText::new(
            "Glob as an operand of [ ] (ShellCheck SC2202)",
            "use a loop",
            "Each match becomes a separate argument of the test.",
            "[ *.txt ] gets too many arguments with several files.",
        ),
    ),
    (
        ShellRule::Sc2203,
        RuleText::new(
            "Glob in [[ ]] where it is not expanded (ShellCheck SC2203)",
            "use a loop",
            "Globs only work to the right of = and != in [[ ]].",
            "[[ *.txt == a ]] compares the text *.txt.",
        ),
    ),
    (
        ShellRule::Sc2208,
        RuleText::new(
            "-v with a glob in [ ] (ShellCheck SC2208)",
            "use [[ ]] or quote the argument",
            "The name can expand to files in the current directory.",
            "[ -v a[0] ] becomes [[ -v a[0] ]].",
        ),
    ),
    (
        ShellRule::Sc2245,
        RuleText::new(
            "File test on a glob in ksh, which only looks at the first match (ShellCheck SC2245)",
            "use a loop to check any or all of them",
            "The test applies only to the first file of the expansion.",
            "[ -d foo* ] only checks one directory.",
        ),
    ),
    (
        ShellRule::Sc2255,
        RuleText::new(
            "Numeric comparison with a glob in [ ] (ShellCheck SC2255)",
            "use $((..)) for numbers, or a string comparison",
            "[ ] does not evaluate arithmetic, so the * is a glob.",
            "[ 6 -eq 2*3 ] becomes [ 6 -eq $((2*3)) ].",
        ),
    ),
    (
        ShellRule::Sc2268,
        RuleText::new(
            "x prefix in a comparison (ShellCheck SC2268)",
            "compare the values directly",
            "The shells that needed it have not been around for decades.",
            "[ x$a = xb ] becomes [ \"$a\" = b ].",
        ),
    ),
];
