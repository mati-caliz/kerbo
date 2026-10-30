use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// The checks of ShellCheck's Checks/Commands.hs on shell builtins.
pub const SHELL_BUILTINS_TEXTS: [(ShellRule, RuleText); 18] = [
    (
        ShellRule::Sc2121,
        RuleText::new(
            "set used to assign a variable (ShellCheck SC2121)",
            "assign with var=value",
            "set changes the positional parameters and the shell options, it does not assign variables.",
            "set name=value becomes name=value.",
        ),
    ),
    (
        ShellRule::Sc2139,
        RuleText::new(
            "alias with an expansion that happens when it is defined (ShellCheck SC2139)",
            "use single quotes or escape the $ so it expands when used",
            "In double quotes the variable is read once, when the alias is created.",
            "alias now=\"date $FORMAT\" becomes alias now='date $FORMAT'.",
        ),
    ),
    (
        ShellRule::Sc2142,
        RuleText::new(
            "alias that uses positional parameters (ShellCheck SC2142)",
            "use a function",
            "Aliases are plain text replacement: $1 refers to the arguments of the surrounding script, not of the alias.",
            "alias g='grep $1 file' becomes g() { grep \"$1\" file; }.",
        ),
    ),
    (
        ShellRule::Sc2151,
        RuleText::new(
            "return with more than one argument (ShellCheck SC2151)",
            "return a single status from 0 to 255 and print other data",
            "return only takes one status code; the rest is an error.",
            "return 1 \"failed\" becomes echo failed >&2; return 1.",
        ),
    ),
    (
        ShellRule::Sc2152,
        RuleText::new(
            "return with something that is not a status from 0 to 255 (ShellCheck SC2152)",
            "return a number from 0 to 255 and print other data to stdout",
            "A function's status is a byte: strings fail and larger numbers wrap around.",
            "return \"$name\" becomes echo \"$name\"; return 0.",
        ),
    ),
    (
        ShellRule::Sc2163,
        RuleText::new(
            "export of $var instead of the variable's name (ShellCheck SC2163)",
            "drop the $: export var",
            "export $var exports the variable whose name is the value of var.",
            "export $PATH becomes export PATH.",
        ),
    ),
    (
        ShellRule::Sc2168,
        RuleText::new(
            "local outside a function (ShellCheck SC2168)",
            "drop local, or move the code into a function",
            "local only works inside a function; elsewhere it is an error.",
            "local x=1 at the top level becomes x=1.",
        ),
    ),
    (
        ShellRule::Sc2172,
        RuleText::new(
            "trap of a signal by number (ShellCheck SC2172)",
            "use the signal name",
            "Signal numbers other than the classic ones differ between systems.",
            "trap cleanup 10 becomes trap cleanup USR1.",
        ),
    ),
    (
        ShellRule::Sc2173,
        RuleText::new(
            "trap of SIGKILL or SIGSTOP, which cannot be trapped (ShellCheck SC2173)",
            "drop that signal from the trap",
            "The kernel never delivers KILL or STOP to the process, so the handler never runs.",
            "trap cleanup INT KILL becomes trap cleanup INT.",
        ),
    ),
    (
        ShellRule::Sc2184,
        RuleText::new(
            "Unquoted unset argument that the shell can expand (ShellCheck SC2184)",
            "quote the argument",
            "unset arr[0] can match a file named arr0 and unset that instead.",
            "unset arr[0] becomes unset 'arr[0]'.",
        ),
    ),
    (
        ShellRule::Sc2219,
        RuleText::new(
            "let instead of (( )) (ShellCheck SC2219)",
            "use (( expression ))",
            "(( )) does the same arithmetic without the quoting problems of let's arguments.",
            "let 'n = n + 1' becomes (( n = n + 1 )).",
        ),
    ),
    (
        ShellRule::Sc2229,
        RuleText::new(
            "read into $var instead of the variable's name (ShellCheck SC2229)",
            "drop the $: read var",
            "read $var reads into the variable whose name is the value of var.",
            "read $line becomes read -r line.",
        ),
    ),
    (
        ShellRule::Sc2240,
        RuleText::new(
            ". with arguments, which sh and dash do not pass (ShellCheck SC2240)",
            "set the values as variables before sourcing",
            "In sh and dash the dot command ignores extra arguments: the sourced file does not see them.",
            ". ./lib.sh debug becomes MODE=debug . ./lib.sh.",
        ),
    ),
    (
        ShellRule::Sc2241,
        RuleText::new(
            "exit with more than one argument (ShellCheck SC2241)",
            "exit with a single status from 0 to 255 and print other data",
            "exit only takes one status code; the rest is an error.",
            "exit 1 \"failed\" becomes echo failed >&2; exit 1.",
        ),
    ),
    (
        ShellRule::Sc2242,
        RuleText::new(
            "exit with something that is not a status from 0 to 255 (ShellCheck SC2242)",
            "exit with a number from 0 to 255 and print other data to stderr",
            "The exit status is a byte: strings fail and larger numbers wrap around.",
            "exit -1 becomes exit 1.",
        ),
    ),
    (
        ShellRule::Sc2293,
        RuleText::new(
            "eval of a @Q-quoted array with @ (ShellCheck SC2293)",
            "use * as the index",
            "With @ each element becomes a separate word for eval; * joins the quoted words into one string.",
            "eval \"${arr[@]@Q}\" becomes eval \"${arr[*]@Q}\".",
        ),
    ),
    (
        ShellRule::Sc2294,
        RuleText::new(
            "eval of an array, which loses its quoting (ShellCheck SC2294)",
            "run the array directly, or eval it as a single string",
            "eval joins the elements and re-parses them, so spaces and symbols in them break.",
            "eval \"${cmd[@]}\" becomes \"${cmd[@]}\".",
        ),
    ),
    (
        ShellRule::Sc2313,
        RuleText::new(
            "Unquoted array index in read, which the shell can expand (ShellCheck SC2313)",
            "quote the index",
            "read arr[0] can match a file named arr0 and read into that name instead.",
            "read arr[0] becomes read 'arr[0]'.",
        ),
    ),
];
