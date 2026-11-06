use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// ShellCheck's Analytics.hs checks on the script's flow: loops, exec, break, functions passed to other commands and the shebang.
pub const CONTROL_TEXTS: [(ShellRule, RuleText); 35] = [
    (
        ShellRule::Sc2000,
        RuleText::new(
            "echo into wc to measure a variable (ShellCheck SC2000)",
            "use ${#variable}",
            "An expansion measures it without starting two processes.",
            "$(echo \"$a\" | wc -c) becomes ${#a}.",
        ),
    ),
    (
        ShellRule::Sc2014,
        RuleText::new(
            "Glob or substitution in find -exec (ShellCheck SC2014)",
            "quote it, or run it through sh -c",
            "It expands once before find runs, not per file found.",
            "find . -exec echo * \\; becomes find . -exec echo '*' \\;.",
        ),
    ),
    (
        ShellRule::Sc2032,
        RuleText::new(
            "Function that a later external command cannot run (ShellCheck SC2032)",
            "use a separate script or sh -c",
            "Functions only exist in the current shell.",
            "foo() { :; }; sudo foo needs a script.",
        ),
    ),
    (
        ShellRule::Sc2033,
        RuleText::new(
            "Function passed to an external command (ShellCheck SC2033)",
            "use a separate script or sh -c",
            "sudo, xargs and the like run programs: they do not see the shell's functions.",
            "foo() { :; }; sudo foo needs a script.",
        ),
    ),
    (
        ShellRule::Sc2036,
        RuleText::new(
            "Assignment at the start of a pipeline (ShellCheck SC2036)",
            "use a=$(b | c)",
            "The variable does not receive the pipeline's output.",
            "a=b | c becomes a=$(b | c).",
        ),
    ),
    (
        ShellRule::Sc2041,
        RuleText::new(
            "for over a single-quoted string (ShellCheck SC2041)",
            "use $(..) to run a command",
            "It is a literal text, so the loop runs once.",
            "for f in 'ls' becomes for f in $(ls).",
        ),
    ),
    (
        ShellRule::Sc2042,
        RuleText::new(
            "Commas separating for values (ShellCheck SC2042)",
            "separate the values with spaces",
            "The commas stay in the value, and the loop runs once.",
            "for i in 1,2 becomes for i in 1 2.",
        ),
    ),
    (
        ShellRule::Sc2043,
        RuleText::new(
            "for that only runs once (ShellCheck SC2043)",
            "fix the quoting, or add the missing glob or expansion",
            "A loop over one fixed value probably lost something.",
            "for f in foo becomes for f in foo*.",
        ),
    ),
    (
        ShellRule::Sc2066,
        RuleText::new(
            "for over something quoted that was meant to split (ShellCheck SC2066)",
            "remove the quotes, or iterate over an array",
            "Quoted, it does not split: the loop runs once.",
            "for f in \"$(ls)\" becomes for f in *.",
        ),
    ),
    (
        ShellRule::Sc2067,
        RuleText::new(
            "find -exec without its terminator (ShellCheck SC2067)",
            "end it with \\; or +",
            "Without it find fails: |, && and an unquoted ; do not end -exec.",
            "find . -exec cmd {} ; becomes find . -exec cmd {} \\;.",
        ),
    ),
    (
        ShellRule::Sc2083,
        RuleText::new(
            "Space after ./ (ShellCheck SC2083)",
            "remove the space",
            "./ is a directory, not the program.",
            "./ foo becomes ./foo.",
        ),
    ),
    (
        ShellRule::Sc2087,
        RuleText::new(
            "Unquoted here document passed to ssh (ShellCheck SC2087)",
            "quote the delimiter",
            "Expansions happen on the client, not on the server.",
            "ssh host << EOF becomes ssh host << 'EOF'.",
        ),
    ),
    (
        ShellRule::Sc2093,
        RuleText::new(
            "Commands after exec never run (ShellCheck SC2093)",
            "remove exec if the script has to continue",
            "exec replaces the shell with the command.",
            "exec foo; bar becomes foo; bar.",
        ),
    ),
    (
        ShellRule::Sc2095,
        RuleText::new(
            "Command that swallows the input of a while read (ShellCheck SC2095)",
            "add the flag that disconnects it from stdin, or redirect it",
            "ssh and the like read the loop's stdin, so it ends after the first line.",
            "ssh host cmd becomes ssh -n host cmd.",
        ),
    ),
    (
        ShellRule::Sc2096,
        RuleText::new(
            "Shebang with more than one parameter (ShellCheck SC2096)",
            "leave one parameter, or use env -S",
            "Most systems pass all of it as one single argument.",
            "#!/bin/bash -e -x becomes #!/bin/bash -ex.",
        ),
    ),
    (
        ShellRule::Sc2097,
        RuleText::new(
            "Prefix assignment only seen by the process (ShellCheck SC2097)",
            "assign on a separate line",
            "The expansion on the same command runs before the assignment takes effect.",
            "a=1 echo $a becomes a=1; echo \"$a\".",
        ),
    ),
    (
        ShellRule::Sc2098,
        RuleText::new(
            "Expansion that does not see the assignment before the command (ShellCheck SC2098)",
            "assign on a separate line",
            "The expansion happens before the assignment takes effect.",
            "a=1 echo $a becomes a=1; echo \"$a\".",
        ),
    ),
    (
        ShellRule::Sc2101,
        RuleText::new(
            "Named character class without its outer brackets (ShellCheck SC2101)",
            "use [[:digit:]]",
            "[:digit:] is a set of the letters d, i, g and t.",
            "ls [:digit:] becomes ls [[:digit:]].",
        ),
    ),
    (
        ShellRule::Sc2102,
        RuleText::new(
            "Glob range with repeated characters (ShellCheck SC2102)",
            "use a list or an alternation for several characters",
            "A range matches one character, not a word.",
            "ls [10-15] matches one character, 0 or 1 to 5.",
        ),
    ),
    (
        ShellRule::Sc2104,
        RuleText::new(
            "break or continue directly in a function (ShellCheck SC2104)",
            "use return",
            "Without a loop around it there is nothing to leave.",
            "f() { break; } becomes f() { return; }.",
        ),
    ),
    (
        ShellRule::Sc2105,
        RuleText::new(
            "break or continue outside a loop (ShellCheck SC2105)",
            "put it inside a loop, or use exit",
            "The shell rejects or ignores it.",
            "break becomes exit.",
        ),
    ),
    (
        ShellRule::Sc2106,
        RuleText::new(
            "break or continue inside a subshell in a loop (ShellCheck SC2106)",
            "move it out of the subshell, or avoid the pipeline",
            "It only ends the subshell, not the loop.",
            "(break) becomes break.",
        ),
    ),
    (
        ShellRule::Sc2111,
        RuleText::new(
            "function and () at the same time in ksh (ShellCheck SC2111)",
            "use one of the two forms",
            "ksh does not allow both.",
            "function f() { :; } becomes f() { :; }.",
        ),
    ),
    (
        ShellRule::Sc2112,
        RuleText::new(
            "function keyword in sh (ShellCheck SC2112)",
            "remove the keyword",
            "It is not standard.",
            "function f() { :; } becomes f() { :; }.",
        ),
    ),
    (
        ShellRule::Sc2113,
        RuleText::new(
            "function keyword without () in sh (ShellCheck SC2113)",
            "use foo() instead",
            "It is not standard.",
            "function f { :; } becomes f() { :; }.",
        ),
    ),
    (
        ShellRule::Sc2118,
        RuleText::new(
            "|& in ksh (ShellCheck SC2118)",
            "use 2>&1 |",
            "In ksh, |& starts a coprocess, not a pipe of stderr.",
            "a |& b becomes a 2>&1 | b.",
        ),
    ),
    (
        ShellRule::Sc2123,
        RuleText::new(
            "PATH overwritten with something that is not a search path (ShellCheck SC2123)",
            "use another name for the variable",
            "PATH decides where commands are looked for: the following ones stop being found.",
            "PATH=/my/dir becomes MY_DIR=/my/dir.",
        ),
    ),
    (
        ShellRule::Sc2127,
        RuleText::new(
            "Case terminator this shell does not support (ShellCheck SC2127)",
            "use ;; or change the shebang",
            "sh and dash only know ;;, and ;;& only exists in bash.",
            "a) b;& becomes a) b;;.",
        ),
    ),
    (
        ShellRule::Sc2147,
        RuleText::new(
            "Quoted tilde in PATH (ShellCheck SC2147)",
            "use $HOME",
            "Several programs do not expand it there.",
            "PATH=\"~/bin:$PATH\" becomes PATH=\"$HOME/bin:$PATH\".",
        ),
    ),
    (
        ShellRule::Sc2165,
        RuleText::new(
            "Nested loop that overwrites the parent's variable (ShellCheck SC2165)",
            "use another variable",
            "The parent loop continues with the inner loop's value.",
            "for i in a; do for i in b; do :; done; done uses j inside.",
        ),
    ),
    (
        ShellRule::Sc2167,
        RuleText::new(
            "Loop whose variable a nested loop overwrites (ShellCheck SC2167)",
            "use another variable in the nested loop",
            "This loop continues with the inner loop's value.",
            "for i in a; do for i in b; do :; done; done uses j inside.",
        ),
    ),
    (
        ShellRule::Sc2187,
        RuleText::new(
            "ash shebang checked as dash (ShellCheck SC2187)",
            "add # shellcheck shell=dash",
            "The checks use dash's rules for ash.",
            "#!/bin/ash needs # shellcheck shell=dash.",
        ),
    ),
    (
        ShellRule::Sc2239,
        RuleText::new(
            "Shebang without an absolute path (ShellCheck SC2239)",
            "use the absolute path of the interpreter",
            "The system looks for the interpreter exactly where it says.",
            "#!bash becomes #!/bin/bash.",
        ),
    ),
    (
        ShellRule::Sc2246,
        RuleText::new(
            "Shebang that points to a directory (ShellCheck SC2246)",
            "point to the interpreter's file",
            "A directory cannot run the script.",
            "#!/bin/ becomes #!/bin/sh.",
        ),
    ),
    (
        ShellRule::Sc2258,
        RuleText::new(
            "Trailing comma in a for value (ShellCheck SC2258)",
            "remove the comma or quote the value",
            "The comma is part of the value, not a separator.",
            "for i in a, b becomes for i in a b.",
        ),
    ),
];
