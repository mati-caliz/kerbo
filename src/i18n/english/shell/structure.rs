use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// More ShellCheck Analytics.hs checks without data flow: test operands, case, subshells, redirections and assignments.
pub const STRUCTURE_TEXTS: [(ShellRule, RuleText); 24] = [
    (
        ShellRule::Sc2001,
        RuleText::new(
            "sed over a variable where a parameter expansion is enough (ShellCheck SC2001)",
            "use ${variable//search/replace}",
            "A parameter expansion does not start a process for a simple replacement.",
            "$(echo \"$a\" | sed 's/x/y/g') becomes ${a//x/y}.",
        ),
    ),
    (
        ShellRule::Sc2035,
        RuleText::new(
            "Glob that can expand to names that look like options (ShellCheck SC2035)",
            "use ./*glob* or -- *glob*",
            "A file called -rf would be read as an option by the command.",
            "rm *.txt becomes rm ./*.txt.",
        ),
    ),
    (
        ShellRule::Sc2037,
        RuleText::new(
            "Assignment followed by a flag or a glob: the value is not the command's output (ShellCheck SC2037)",
            "use var=$(cmd) to assign the output",
            "A=ls -l sets A only for the command -l, which does not exist.",
            "A=ls -l becomes A=$(ls -l).",
        ),
    ),
    (
        ShellRule::Sc2044,
        RuleText::new(
            "for loop over the output of find (ShellCheck SC2044)",
            "use find -exec or a while read loop",
            "Names with spaces or newlines get split into several words.",
            "for f in $(find .) becomes find . -exec cmd {} +.",
        ),
    ),
    (
        ShellRule::Sc2045,
        RuleText::new(
            "for loop over the output of ls (ShellCheck SC2045)",
            "use a glob",
            "Names with spaces get split, and ls output is meant for people.",
            "for f in $(ls *.txt) becomes for f in *.txt.",
        ),
    ),
    (
        ShellRule::Sc2079,
        RuleText::new(
            "Decimal number in an arithmetic context (ShellCheck SC2079)",
            "use bc or awk",
            "Shell arithmetic only works with integers.",
            "a[1.5]=x needs an integer index.",
        ),
    ),
    (
        ShellRule::Sc2094,
        RuleText::new(
            "The same file is read and written in one pipeline (ShellCheck SC2094)",
            "write to a temporary file and move it afterwards",
            "The redirection truncates the file before the command reads it.",
            "cat f > f ends up with an empty f.",
        ),
    ),
    (
        ShellRule::Sc2103,
        RuleText::new(
            "cd followed by another cd to go back (ShellCheck SC2103)",
            "use a ( subshell ) instead of going back by hand",
            "If the first cd fails, the second one moves to the wrong directory.",
            "cd d; x; cd .. becomes (cd d && x).",
        ),
    ),
    (
        ShellRule::Sc2129,
        RuleText::new(
            "Several commands in a row append to the same file (ShellCheck SC2129)",
            "use { cmd1; cmd2; } >> file",
            "The file is opened once instead of once per command.",
            "a >> f; b >> f; c >> f becomes { a; b; c; } >> f.",
        ),
    ),
    (
        ShellRule::Sc2141,
        RuleText::new(
            "IFS with a literal backslash, n or t (ShellCheck SC2141)",
            "use $'\\n', $'\\t' or a literal character",
            "Between plain quotes \\n is a backslash and an n, so words split at those letters.",
            "IFS=\"\\n\" becomes IFS=$'\\n'.",
        ),
    ),
    (
        ShellRule::Sc2194,
        RuleText::new(
            "case on a constant word (ShellCheck SC2194)",
            "add the $ that is missing on the variable",
            "The same branch runs every time.",
            "case foo in becomes case $foo in.",
        ),
    ),
    (
        ShellRule::Sc2195,
        RuleText::new(
            "case pattern that can never match the word (ShellCheck SC2195)",
            "check the pattern and the word",
            "That branch never runs.",
            "case x$a in y*) never matches.",
        ),
    ),
    (
        ShellRule::Sc2209,
        RuleText::new(
            "A command name assigned to a variable (ShellCheck SC2209)",
            "use var=$(command) for its output, or quote it for the text",
            "It looks like the output of the command was meant.",
            "here=pwd becomes here=$(pwd).",
        ),
    ),
    (
        ShellRule::Sc2221,
        RuleText::new(
            "case pattern that always overrides a later one (ShellCheck SC2221)",
            "move the most specific pattern first",
            "The later branch never runs.",
            "*.txt) before a.txt) leaves the second one dead.",
        ),
    ),
    (
        ShellRule::Sc2222,
        RuleText::new(
            "case pattern that never matches because of an earlier one (ShellCheck SC2222)",
            "move this pattern before the earlier one, or remove it",
            "An earlier pattern already covers everything this one matches.",
            "a.txt) after *.txt) never runs.",
        ),
    ),
    (
        ShellRule::Sc2231,
        RuleText::new(
            "Unquoted expansion in a for loop glob (ShellCheck SC2231)",
            "quote the expansion, e.g. \"$dir\"/*.txt",
            "The directory gets split on spaces before the glob is applied.",
            "for f in $d/*.txt becomes for f in \"$d\"/*.txt.",
        ),
    ),
    (
        ShellRule::Sc2233,
        RuleText::new(
            "Parentheses of more around a condition (ShellCheck SC2233)",
            "remove the ( )",
            "They start a subshell just to evaluate a test.",
            "if ( [ a ] ); then becomes if [ a ]; then.",
        ),
    ),
    (
        ShellRule::Sc2234,
        RuleText::new(
            "Parentheses of more around a test command (ShellCheck SC2234)",
            "remove the ( )",
            "They start a subshell just to evaluate a test.",
            "( [ a ] ) becomes [ a ].",
        ),
    ),
    (
        ShellRule::Sc2235,
        RuleText::new(
            "Subshell that only groups tests (ShellCheck SC2235)",
            "use { ..; } instead of (..)",
            "Braces group the same without starting a subshell.",
            "( [ a ] || [ b ] ) becomes { [ a ] || [ b ]; }.",
        ),
    ),
    (
        ShellRule::Sc2269,
        RuleText::new(
            "Variable assigned to itself (ShellCheck SC2269)",
            "remove the assignment",
            "The assignment does nothing.",
            "x=$x goes away.",
        ),
    ),
    (
        ShellRule::Sc2295,
        RuleText::new(
            "Unquoted expansion inside a ${..} pattern (ShellCheck SC2295)",
            "quote the expansion separately",
            "Otherwise its value matches as a pattern and not as text.",
            "${a%$b} becomes ${a%\"$b\"}.",
        ),
    ),
    (
        ShellRule::Sc2321,
        RuleText::new(
            "$(( )) inside an array index (ShellCheck SC2321)",
            "remove the $(( and the ))",
            "Indexes of normal arrays are already an arithmetic context.",
            "a[$((i+1))]=x becomes a[i+1]=x.",
        ),
    ),
    (
        ShellRule::Sc2327,
        RuleText::new(
            "Command substitution whose output is redirected away (ShellCheck SC2327)",
            "remove the redirection, or use tee",
            "The substitution always ends up empty.",
            "x=$(cmd > f) leaves x empty.",
        ),
    ),
    (
        ShellRule::Sc2328,
        RuleText::new(
            "Redirection takes the output away from a command substitution (ShellCheck SC2328)",
            "use tee to duplicate the output",
            "Nothing reaches the substitution.",
            "x=$(cmd > f) becomes x=$(cmd | tee f).",
        ),
    ),
];
