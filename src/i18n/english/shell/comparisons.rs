use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// ShellCheck's Analytics.hs checks on what gets compared and how: test operators, conditions chained with && and ||, arithmetic and commands that look like comparisons.
pub const COMPARISONS_TEXTS: [(ShellRule, RuleText); 43] = [
    (
        ShellRule::Sc2017,
        RuleText::new(
            "Division before multiplication loses precision (ShellCheck SC2017)",
            "multiply first: a*c/b",
            "Integer division truncates before the multiplication happens.",
            "$((a/b*c)) becomes $((a*c/b)).",
        ),
    ),
    (
        ShellRule::Sc2049,
        RuleText::new(
            "=~ with what looks like a glob (ShellCheck SC2049)",
            "use = for globs, or write a real regular expression",
            "=~ reads the right side as a regular expression, where * does not mean any text.",
            "[[ $a =~ *.txt ]] becomes [[ $a = *.txt ]].",
        ),
    ),
    (
        ShellRule::Sc2053,
        RuleText::new(
            "Unquoted right side of a comparison in [[ ]] (ShellCheck SC2053)",
            "quote the right side",
            "Unquoted, the value of the variable is read as a glob, not as text.",
            "[[ $a = $b ]] becomes [[ $a = \"$b\" ]].",
        ),
    ),
    (
        ShellRule::Sc2054,
        RuleText::new(
            "Commas separating array elements (ShellCheck SC2054)",
            "separate the elements with spaces",
            "The comma stays inside the element, because arrays are split on spaces.",
            "a=(1, 2) becomes a=(1 2).",
        ),
    ),
    (
        ShellRule::Sc2055,
        RuleText::new(
            "Two different values compared with != joined by or (ShellCheck SC2055)",
            "use -a or && instead of -o or ||",
            "A value always differs from one of two different values, so the condition is always true.",
            "[ $a != x -o $a != y ] becomes [ $a != x -a $a != y ].",
        ),
    ),
    (
        ShellRule::Sc2056,
        RuleText::new(
            "Two != comparisons of the same value joined by || in arithmetic (ShellCheck SC2056)",
            "use && instead of ||",
            "A value always differs from one of two different values, so the condition is always true.",
            "((a != 1 || a != 2)) becomes ((a != 1 && a != 2)).",
        ),
    ),
    (
        ShellRule::Sc2057,
        RuleText::new(
            "Unknown binary operator in a condition (ShellCheck SC2057)",
            "use an operator that exists",
            "The shell rejects the condition when it runs.",
            "[ $a -foo $b ] becomes [ $a -eq $b ].",
        ),
    ),
    (
        ShellRule::Sc2058,
        RuleText::new(
            "Unknown unary operator in a condition (ShellCheck SC2058)",
            "use an operator that exists",
            "The shell rejects the condition when it runs.",
            "[ -foo $a ] becomes [ -f $a ].",
        ),
    ),
    (
        ShellRule::Sc2065,
        RuleText::new(
            "> or < after test is a file redirection (ShellCheck SC2065)",
            "use -gt or -lt, or escape the operator",
            "The shell creates or reads a file with that name and test never sees the comparison.",
            "test 1 > 2 becomes test 1 -gt 2.",
        ),
    ),
    (
        ShellRule::Sc2071,
        RuleText::new(
            "String operator comparing numbers (ShellCheck SC2071)",
            "use -lt, -gt, -le or -ge",
            "< and > compare text: 10 comes before 9.",
            "[[ $a > 3 ]] becomes [[ $a -gt 3 ]].",
        ),
    ),
    (
        ShellRule::Sc2072,
        RuleText::new(
            "Decimal number in a comparison (ShellCheck SC2072)",
            "use integers, or compare with bc or awk",
            "The shell only compares integers.",
            "[ $a -lt 1.5 ] becomes a comparison in awk.",
        ),
    ),
    (
        ShellRule::Sc2073,
        RuleText::new(
            "Unescaped < or > in [ ] (ShellCheck SC2073)",
            "escape the operator or use [[ ]]",
            "Unescaped, it redirects to a file instead of comparing.",
            "[ a < b ] becomes [ a \\< b ].",
        ),
    ),
    (
        ShellRule::Sc2074,
        RuleText::new(
            "=~ in [ ] (ShellCheck SC2074)",
            "use [[ ]]",
            "=~ only exists in [[ ]].",
            "[ $a =~ b ] becomes [[ $a =~ b ]].",
        ),
    ),
    (
        ShellRule::Sc2080,
        RuleText::new(
            "Number with a leading 0 that is not valid octal (ShellCheck SC2080)",
            "remove the 0, or write 10#08",
            "A leading 0 means octal, where 8 and 9 do not exist.",
            "((a = 08)) becomes ((a = 8)).",
        ),
    ),
    (
        ShellRule::Sc2081,
        RuleText::new(
            "Glob in a [ ] comparison (ShellCheck SC2081)",
            "use [[ ]] or a case statement",
            "[ ] compares text: the glob is not expanded as a pattern.",
            "[ $a = *.txt ] becomes [[ $a = *.txt ]].",
        ),
    ),
    (
        ShellRule::Sc2099,
        RuleText::new(
            "Arithmetic written as a command after an assignment (ShellCheck SC2099)",
            "use $((..))",
            "The shell runs + as a command with a temporary variable.",
            "i=$i + 1 becomes i=$((i + 1)).",
        ),
    ),
    (
        ShellRule::Sc2100,
        RuleText::new(
            "Arithmetic assigned as text (ShellCheck SC2100)",
            "use $((..))",
            "The variable receives the text, without calculating anything.",
            "i=i+1 becomes i=$((i + 1)).",
        ),
    ),
    (
        ShellRule::Sc2107,
        RuleText::new(
            "&& inside [ ] (ShellCheck SC2107)",
            "use [ a ] && [ b ]",
            "[ ] does not know && and the shell cuts the command there.",
            "[ a && b ] becomes [ a ] && [ b ].",
        ),
    ),
    (
        ShellRule::Sc2108,
        RuleText::new(
            "-a inside [[ ]] (ShellCheck SC2108)",
            "use &&",
            "[[ ]] does not accept -a as and.",
            "[[ a -a b ]] becomes [[ a && b ]].",
        ),
    ),
    (
        ShellRule::Sc2109,
        RuleText::new(
            "|| inside [ ] (ShellCheck SC2109)",
            "use [ a ] || [ b ]",
            "[ ] does not know || and the shell cuts the command there.",
            "[ a || b ] becomes [ a ] || [ b ].",
        ),
    ),
    (
        ShellRule::Sc2110,
        RuleText::new(
            "-o inside [[ ]] (ShellCheck SC2110)",
            "use ||",
            "[[ ]] does not accept -o as or.",
            "[[ a -o b ]] becomes [[ a || b ]].",
        ),
    ),
    (
        ShellRule::Sc2122,
        RuleText::new(
            "<= or >= in a text comparison (ShellCheck SC2122)",
            "use ! a > b, or -le and -ge for numbers",
            "Those operators do not exist for text.",
            "[[ a <= b ]] becomes [[ ! a > b ]].",
        ),
    ),
    (
        ShellRule::Sc2170,
        RuleText::new(
            "Text where [ ] expects a number (ShellCheck SC2170)",
            "use = to compare text, or $var for a variable",
            "-eq and the like fail if the operand is not a number.",
            "[ $a -eq foo ] becomes [ $a = foo ].",
        ),
    ),
    (
        ShellRule::Sc2171,
        RuleText::new(
            "Closing ] or ]] without its opening (ShellCheck SC2171)",
            "add the opening bracket, or quote it if it is literal",
            "The closing bracket is passed as a plain argument: the command is not a test.",
            "echo a ] becomes [ a ].",
        ),
    ),
    (
        ShellRule::Sc2204,
        RuleText::new(
            "( ) with a test operator (ShellCheck SC2204)",
            "use [ ] for a test",
            "( ) opens a subshell and runs -e as a command.",
            "( -e file ) becomes [ -e file ].",
        ),
    ),
    (
        ShellRule::Sc2205,
        RuleText::new(
            "( ) with a comparison operator (ShellCheck SC2205)",
            "use [ ] for a test",
            "( ) opens a subshell and runs the first word as a command.",
            "( a = b ) becomes [ a = b ].",
        ),
    ),
    (
        ShellRule::Sc2212,
        RuleText::new(
            "Empty [ ] (ShellCheck SC2212)",
            "use false",
            "An empty test is an obscure way of writing false.",
            "[ ] becomes false.",
        ),
    ),
    (
        ShellRule::Sc2252,
        RuleText::new(
            "Two tests of != with different values joined by || (ShellCheck SC2252)",
            "use && instead of ||",
            "A value always differs from one of two different values, so the result is always true.",
            "[ $a != x ] || [ $a != y ] becomes [ $a != x ] && [ $a != y ].",
        ),
    ),
    (
        ShellRule::Sc2254,
        RuleText::new(
            "Unquoted expansion in a case pattern (ShellCheck SC2254)",
            "quote the expansion",
            "Unquoted, the value is read as a pattern and not as text.",
            "case $a in $b) becomes case $a in \"$b\").",
        ),
    ),
    (
        ShellRule::Sc2257,
        RuleText::new(
            "Arithmetic that modifies a variable inside a redirection (ShellCheck SC2257)",
            "do the modification in a separate statement",
            "The shell may discard that change.",
            "cmd > $((i++)) becomes cmd > \"$i\"; ((i++)).",
        ),
    ),
    (
        ShellRule::Sc2265,
        RuleText::new(
            "& after a test (ShellCheck SC2265)",
            "use &&",
            "A single & sends the test to the background and always continues.",
            "[ a ] & [ b ] becomes [ a ] && [ b ].",
        ),
    ),
    (
        ShellRule::Sc2266,
        RuleText::new(
            "| next to a test (ShellCheck SC2266)",
            "use ||",
            "A single | pipes the output instead of working as or.",
            "[ a ] | [ b ] becomes [ a ] || [ b ].",
        ),
    ),
    (
        ShellRule::Sc2283,
        RuleText::new(
            "= as the second word of a command (ShellCheck SC2283)",
            "remove the spaces to assign, or use [ ] to compare",
            "The shell runs the first word as a command, with = as its argument.",
            "a = 1 becomes a=1.",
        ),
    ),
    (
        ShellRule::Sc2284,
        RuleText::new(
            "== as the second word of a command (ShellCheck SC2284)",
            "use [ a = b ] to compare",
            "The shell runs the first word as a command, with == as its argument.",
            "a == 1 becomes [ \"$a\" = 1 ].",
        ),
    ),
    (
        ShellRule::Sc2285,
        RuleText::new(
            "+= as the second word of a command (ShellCheck SC2285)",
            "remove the spaces around +=",
            "The shell runs the first word as a command, with += as its argument.",
            "a += 1 becomes a+=1.",
        ),
    ),
    (
        ShellRule::Sc2309,
        RuleText::new(
            "-eq and the like read text as arithmetic in [[ ]] (ShellCheck SC2309)",
            "use = to compare text, or expand it explicitly with $var",
            "A word that is not a number is evaluated as a variable or arithmetic.",
            "[[ $a -eq foo ]] becomes [[ $a = foo ]].",
        ),
    ),
    (
        ShellRule::Sc2322,
        RuleText::new(
            "Double parentheses inside an arithmetic expression (ShellCheck SC2322)",
            "leave a single layer of parentheses",
            "((x)) is the same as (x).",
            "$(( ((a)) + 1 )) becomes $(( (a) + 1 )).",
        ),
    ),
    (
        ShellRule::Sc2323,
        RuleText::new(
            "Parentheses around a whole arithmetic expression (ShellCheck SC2323)",
            "remove the extra parentheses",
            "$(( (x) )) is the same as $(( x )).",
            "$(( (a + 1) )) becomes $(( a + 1 )).",
        ),
    ),
    (
        ShellRule::Sc2324,
        RuleText::new(
            "+= with a number on a variable that is not an integer (ShellCheck SC2324)",
            "use (( var += 1 )) or typeset -i var, or quote the number",
            "+= appends text: 1 += 1 gives 11.",
            "a+=1 becomes (( a += 1 )).",
        ),
    ),
    (
        ShellRule::Sc2330,
        RuleText::new(
            "Glob in [[ ]] on BusyBox (ShellCheck SC2330)",
            "use a case statement",
            "BusyBox's [[ ]] does not match patterns.",
            "[[ $a = *.txt ]] becomes case \"$a\" in *.txt).",
        ),
    ),
    (
        ShellRule::Sc2331,
        RuleText::new(
            "-a to check that a file exists (ShellCheck SC2331)",
            "use -e",
            "-a is the old form of -e and is easy to mistake for and.",
            "[ -a file ] becomes [ -e file ].",
        ),
    ),
    (
        ShellRule::Sc2333,
        RuleText::new(
            "Two = comparisons of the same value joined by and (ShellCheck SC2333)",
            "use -o or || instead",
            "A value cannot be equal to two different values, so the condition is always false.",
            "[ $a = x ] && [ $a = y ] becomes [ $a = x ] || [ $a = y ].",
        ),
    ),
    (
        ShellRule::Sc2334,
        RuleText::new(
            "Two == comparisons of the same value joined by && in arithmetic (ShellCheck SC2334)",
            "use || instead of &&",
            "A value cannot be equal to two different numbers, so the condition is always false.",
            "((a == 1 && a == 2)) becomes ((a == 1 || a == 2)).",
        ),
    ),
];
