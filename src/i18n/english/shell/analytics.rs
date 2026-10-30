use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// The checks of ShellCheck's Analytics.hs that need no data flow, and the source commands its parser does not follow.
pub const ANALYTICS_TEXTS: [(ShellRule, RuleText); 24] = [
    (
        ShellRule::Sc1090,
        RuleText::new(
            "source of a path ShellCheck cannot know (ShellCheck SC1090)",
            "add a # shellcheck source=path directive, or source=/dev/null if it does not matter",
            "Without a constant path the sourced file is not checked, and the variables it defines look undefined.",
            "source \"$1\" gets # shellcheck source=lib.sh before it.",
        ),
    ),
    (
        ShellRule::Sc1091,
        RuleText::new(
            "source of a file that is not checked with the script (ShellCheck SC1091)",
            "pass the file to the check too, or add # shellcheck source=/dev/null",
            "The sourced file is not followed, so what it defines is unknown to the rest of the checks.",
            "source ./lib.sh is checked together with lib.sh, or gets source=/dev/null.",
        ),
    ),
    (
        ShellRule::Sc2007,
        RuleText::new(
            "$[..] arithmetic, which is deprecated (ShellCheck SC2007)",
            "use $((..))",
            "$[..] is an old bash form that newer shells may drop.",
            "$[a+1] becomes $((a+1)).",
        ),
    ),
    (
        ShellRule::Sc2009,
        RuleText::new(
            "ps piped to grep (ShellCheck SC2009)",
            "use pgrep",
            "grep also matches its own process and lines by accident; pgrep matches process names directly.",
            "ps aux | grep nginx becomes pgrep -a nginx.",
        ),
    ),
    (
        ShellRule::Sc2010,
        RuleText::new(
            "ls piped to grep (ShellCheck SC2010)",
            "use a glob or a loop with a condition",
            "Parsing ls breaks with file names that have spaces, newlines or odd characters.",
            "ls | grep '\\.log$' becomes for f in *.log; do ...; done.",
        ),
    ),
    (
        ShellRule::Sc2011,
        RuleText::new(
            "ls piped to xargs (ShellCheck SC2011)",
            "use find -print0 | xargs -0 or find -exec",
            "xargs splits ls output on spaces, so names with spaces become several arguments.",
            "ls | xargs rm becomes find . -maxdepth 1 -type f -exec rm {} +.",
        ),
    ),
    (
        ShellRule::Sc2012,
        RuleText::new(
            "ls output read by another command (ShellCheck SC2012)",
            "use find or a glob",
            "ls output is meant for people: names with newlines or odd characters do not survive it.",
            "ls *.txt | wc -l becomes find . -name '*.txt' | wc -l, or a glob into an array.",
        ),
    ),
    (
        ShellRule::Sc2013,
        RuleText::new(
            "for loop over $(cat file), which splits on words (ShellCheck SC2013)",
            "use while IFS= read -r line; do ...; done < file",
            "for splits the output on spaces and expands globs, so a line with spaces becomes several items.",
            "for l in $(cat f) becomes while IFS= read -r l; do ...; done < f.",
        ),
    ),
    (
        ShellRule::Sc2026,
        RuleText::new(
            "Word left outside single quotes between two quoted parts (ShellCheck SC2026)",
            "nest the quotes as '\"'single quotes'\"' or quote the word too",
            "The word between the two single-quoted parts is outside the quotes and gets split and expanded.",
            "echo 'it'is'fine' keeps is outside the quotes.",
        ),
    ),
    (
        ShellRule::Sc2027,
        RuleText::new(
            "Expansion between two double-quoted parts, which ends up unquoted (ShellCheck SC2027)",
            "remove the inner quotes or escape them",
            "\"a\"$x\"b\" closes the quotes before $x, so $x is split and globbed.",
            "echo \"Hi \"$name\"!\" becomes echo \"Hi $name!\".",
        ),
    ),
    (
        ShellRule::Sc2038,
        RuleText::new(
            "find piped to xargs without null separators (ShellCheck SC2038)",
            "use find -print0 | xargs -0, or find -exec ... +",
            "xargs splits on spaces and newlines, so file names with them break.",
            "find . -name '*.log' | xargs rm becomes find . -name '*.log' -exec rm {} +.",
        ),
    ),
    (
        ShellRule::Sc2048,
        RuleText::new(
            "Unquoted $* or ${array[*]} (ShellCheck SC2048)",
            "use \"$@\" or \"${array[@]}\" with quotes",
            "Unquoted, every argument is split again on spaces and globbed.",
            "cmd $* becomes cmd \"$@\".",
        ),
    ),
    (
        ShellRule::Sc2070,
        RuleText::new(
            "[ -n ] with an unquoted argument (ShellCheck SC2070)",
            "quote the argument or use [[ ]]",
            "If the variable is empty, [ -n ] tests the string -n and is always true.",
            "[ -n $x ] becomes [ -n \"$x\" ].",
        ),
    ),
    (
        ShellRule::Sc2084,
        RuleText::new(
            "$((expr)) as a whole command, which runs the result (ShellCheck SC2084)",
            "drop the $ or assign it with _=$((expr))",
            "The shell runs the number that comes out as a command name.",
            "$((count++)) becomes ((count++)) or : $((count++)).",
        ),
    ),
    (
        ShellRule::Sc2088,
        RuleText::new(
            "~ inside quotes, where it does not expand (ShellCheck SC2088)",
            "use $HOME",
            "The tilde only expands unquoted at the start of a word.",
            "\"~/backup\" becomes \"$HOME/backup\".",
        ),
    ),
    (
        ShellRule::Sc2091,
        RuleText::new(
            "$(cmd) as a whole command, which runs its output (ShellCheck SC2091)",
            "drop the $( ), or use eval if running the output is the intent",
            "The output of the command is run as another command.",
            "if $(grep -q x f); then becomes if grep -q x f; then.",
        ),
    ),
    (
        ShellRule::Sc2092,
        RuleText::new(
            "`cmd` as a whole command, which runs its output (ShellCheck SC2092)",
            "drop the backticks, or use eval if running the output is the intent",
            "The output of the command is run as another command.",
            "`which python` becomes which python.",
        ),
    ),
    (
        ShellRule::Sc2119,
        RuleText::new(
            "Call without arguments to a function that reads $1 (ShellCheck SC2119)",
            "pass the arguments: name \"$@\" if it should see the script's ones",
            "Inside a function $1 is the function's first argument, not the script's.",
            "usage() { echo \"$1\"; }; usage becomes usage \"$@\".",
        ),
    ),
    (
        ShellRule::Sc2120,
        RuleText::new(
            "Function that reads its arguments but is never called with any (ShellCheck SC2120)",
            "pass the arguments in the calls, or use the script's values another way",
            "Every call leaves $1 empty, which is usually a forgotten argument or a mix-up with the script's $1.",
            "greet() { echo \"$1\"; }; greet becomes greet \"$name\".",
        ),
    ),
    (
        ShellRule::Sc2124,
        RuleText::new(
            "Array assigned to a plain variable (ShellCheck SC2124)",
            "assign an array with ( ), or use * instead of @ to join",
            "var=$@ joins the elements and loses where each one ended.",
            "args=$@ becomes args=(\"$@\").",
        ),
    ),
    (
        ShellRule::Sc2125,
        RuleText::new(
            "Glob or brace expansion in an assignment, where it stays literal (ShellCheck SC2125)",
            "quote it, or use an array to expand it",
            "Assignments do not expand globs or braces, so the variable holds the pattern itself.",
            "files=*.txt becomes files=(*.txt).",
        ),
    ),
    (
        ShellRule::Sc2126,
        RuleText::new(
            "grep piped to wc -l (ShellCheck SC2126)",
            "use grep -c",
            "grep counts matching lines by itself.",
            "grep x f | wc -l becomes grep -c x f.",
        ),
    ),
    (
        ShellRule::Sc2140,
        RuleText::new(
            "Word of the form \"A\"B\"C\", with B outside the quotes (ShellCheck SC2140)",
            "write \"ABC\", or escape the inner quotes as \\\"",
            "The middle part ends up unquoted, which is rarely what was meant.",
            "echo \"a\"b\"c\" becomes echo \"abc\" or echo \"a\\\"b\\\"c\".",
        ),
    ),
    (
        ShellRule::Sc2145,
        RuleText::new(
            "Argument that mixes a string and an array (ShellCheck SC2145)",
            "use * to join, or pass the array as a separate argument",
            "\"text $@\" glues the text to the first element and splits the rest into separate words.",
            "echo \"Args: $@\" becomes echo \"Args: $*\".",
        ),
    ),
];
