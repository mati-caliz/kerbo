mod bashism_commands;
mod bashisms;
mod shell_builtins;
mod tools;

use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;
use bashism_commands::BASHISM_COMMANDS_TEXTS;
use bashisms::BASHISMS_TEXTS;
use shell_builtins::SHELL_BUILTINS_TEXTS;
use tools::TOOLS_TEXTS;

const SPLITTING_WHY: &str = "Unquoted, the shell splits the value on every space and expands globs: a file name with a space or an asterisk turns into several arguments.";
const USELESS_ECHO_WHY: &str = "The echo only repeats what you already have: it adds a process and, unquoted, splits and globs the text.";

const BASHISM_WHY: &str = "A script that starts with #!/bin/sh may run under dash, busybox or another POSIX shell, where this bash feature fails or does something else.";

const UNKNOWN: RuleText = RuleText::new(
    "ShellCheck rule",
    "follow the rule's advice",
    "ShellCheck reports it by default.",
    "See the ShellCheck wiki.",
);
const TEXTS: [RuleText; 22] = [
    RuleText::new(
        "Needless $ or ${} on a variable inside an arithmetic expression",
        "drop the $: inside $(( )) variables are read by name",
        "In arithmetic the name already is the variable; with $ the value is pasted as text before evaluating, and an empty value breaks the expression.",
        "$(( $total + 1 )) becomes $(( total + 1 )).",
    ),
    RuleText::new(
        "echo $(command): the echo is useless",
        "run the command directly",
        USELESS_ECHO_WHY,
        "echo \"$(date)\" becomes date.",
    ),
    RuleText::new(
        "Command substitution with backticks",
        "use $(...) instead of `...`",
        "Backticks don't nest without escapes and treat backslashes differently; $(...) reads and nests without surprises.",
        "now=`date` becomes now=$(date).",
    ),
    RuleText::new(
        "A && B || C is not an if: C also runs when B fails",
        "use if A; then B; else C; fi",
        "If A succeeds but B fails, C runs anyway: the shortcut hides a path nobody meant to write.",
        "[ -f x ] && cp x y || echo missing becomes if [ -f x ]; then cp x y; else echo missing; fi.",
    ),
    RuleText::new(
        "An expansion inside single quotes doesn't expand",
        "use double quotes to get the value, or escape the $ to keep the text",
        "Inside single quotes $variable and $(command) stay literal: most likely the value was expected.",
        "echo 'Hello $USER' becomes echo \"Hello $USER\".",
    ),
    RuleText::new(
        "A variable that is assigned and never read",
        "remove it, use it, or export it if another program reads it",
        "An assignment nobody uses is usually a misspelled name or code left over from another version.",
        "If only a script that sources this one reads it, export LOG_LEVEL=debug makes that explicit.",
    ),
    RuleText::new(
        "An unquoted command substitution",
        "put quotes around the $(...)",
        SPLITTING_WHY,
        "rm $(ls *.tmp) becomes rm \"$(ls *.tmp)\", or better a for loop over the glob.",
    ),
    RuleText::new(
        "A variable used as the printf format",
        "use a fixed format and pass the variable as an argument",
        "A % or a backslash in the value is read as formatting: the output changes with the data.",
        "printf \"$message\" becomes printf '%s\\n' \"$message\".",
    ),
    RuleText::new(
        "trap with double quotes: it expands when declared",
        "use single quotes so it expands when the signal arrives",
        "With double quotes the trap command keeps today's values, not the ones present when it runs.",
        "trap \"rm -f $tmp\" EXIT becomes trap 'rm -f \"$tmp\"' EXIT.",
    ),
    RuleText::new(
        "An unquoted array expansion",
        "quote it: \"$@\" or \"${array[@]}\"",
        "Unquoted, each element is split again on spaces and globs: an argument with spaces arrives as several.",
        "command $@ becomes command \"$@\".",
    ),
    RuleText::new(
        "2>&1 before the redirection to a file",
        "put the 2>&1 last: > file 2>&1",
        "Redirections apply in order: 2>&1 first points stderr at the terminal and only then stdout goes to the file.",
        "command 2>&1 > log becomes command > log 2>&1.",
    ),
    RuleText::new(
        "An unquoted variable: it is split into words and globbed",
        "put double quotes around the variable",
        SPLITTING_WHY,
        "cp $source $target becomes cp \"$source\" \"$target\".",
    ),
    RuleText::new(
        "$(echo ...): the echo is useless",
        "use the value directly",
        USELESS_ECHO_WHY,
        "name=$(echo $base) becomes name=$base.",
    ),
    RuleText::new(
        "The script doesn't say which shell runs it",
        "add a shebang on the first line, or a shell= directive",
        "Without a shebang the script runs with whatever shell calls it, and the rules can't tell sh from bash.",
        "First line: #!/usr/bin/env bash",
    ),
    RuleText::new(
        "Declaring and assigning together masks the exit code",
        "declare it on one line and assign it on the next",
        "local, export and declare return their own success: if the command in the assignment fails nobody notices, not even set -e.",
        "local version=$(git describe) becomes local version and then version=$(git describe).",
    ),
    RuleText::new(
        "read without -r mangles backslashes",
        "add -r to the read",
        "Without -r, read eats backslashes: a Windows path or a line with \\n arrives changed.",
        "read line becomes read -r line.",
    ),
    RuleText::new(
        "cd without handling failure",
        "add || exit (or || return inside a function)",
        "If the cd fails the script stays in the previous directory and the next commands run in the wrong place: an rm may delete something else.",
        "cd \"$target\" becomes cd \"$target\" || exit.",
    ),
    RuleText::new(
        "-a or -o inside [ ]",
        "split the condition: [ p ] && [ q ] or [ p ] || [ q ]",
        "POSIX marks -a and -o obsolescent and their precedence is ambiguous: depending on the values, test reads something else.",
        "[ -n \"$a\" -a -n \"$b\" ] becomes [ -n \"$a\" ] && [ -n \"$b\" ].",
    ),
    RuleText::new(
        "Checking $? instead of the command",
        "put the command directly in the if",
        "Another command can slip in between the command and the $? check, and the code gets longer and more fragile.",
        "command; if [ $? -ne 0 ]; then ... becomes if ! command; then ...",
    ),
    RuleText::new(
        "An array built from an unquoted variable",
        "quote it, or split with read -a or mapfile",
        SPLITTING_WHY,
        "parts=( $line ) becomes read -ra parts <<< \"$line\".",
    ),
    RuleText::new(
        "An array built from unquoted command output",
        "use mapfile or read -a to split the output",
        SPLITTING_WHY,
        "files=( $(ls) ) becomes mapfile -t files < <(ls).",
    ),
    RuleText::new(
        "An unquoted default assignment as an argument to :",
        "quote it: : \"${variable:=value}\"",
        "Unquoted, the value is split and globbed: a * in the default walks the whole disk.",
        ": ${DIR:=/tmp} becomes : \"${DIR:=/tmp}\".",
    ),
];

pub fn shell(rule: ShellRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or_else(|| {
        BASHISMS_TEXTS
            .iter()
            .chain(BASHISM_COMMANDS_TEXTS.iter())
            .chain(TOOLS_TEXTS.iter())
            .chain(SHELL_BUILTINS_TEXTS.iter())
            .find(|(candidate, _)| *candidate == rule)
            .map_or(UNKNOWN, |(_, text)| *text)
    })
}
