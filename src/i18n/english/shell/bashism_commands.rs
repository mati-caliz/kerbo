use super::BASHISM_WHY;
use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// The commands and flags of ShellCheck's checkBashisms.
pub const BASHISM_COMMANDS_TEXTS: [(ShellRule, RuleText); 31] = [
    (
        ShellRule::Sc3036,
        RuleText::new(
            "echo flag the shell does not have in a sh script (ShellCheck SC3036)",
            "use printf",
            BASHISM_WHY,
            "echo -e 'a\\tb' becomes printf 'a\\tb\\n'.",
        ),
    ),
    (
        ShellRule::Sc3037,
        RuleText::new(
            "echo flags in a sh script (ShellCheck SC3037)",
            "use printf, which behaves the same everywhere",
            BASHISM_WHY,
            "echo -n \"$x\" becomes printf '%s' \"$x\".",
        ),
    ),
    (
        ShellRule::Sc3038,
        RuleText::new(
            "exec flags in a sh script (ShellCheck SC3038)",
            "run the command without exec's flags, or switch to bash",
            BASHISM_WHY,
            "exec -a name cmd becomes exec cmd.",
        ),
    ),
    (
        ShellRule::Sc3039,
        RuleText::new(
            "let in a sh script (ShellCheck SC3039)",
            "use $((...)) in an assignment",
            BASHISM_WHY,
            "let n=n+1 becomes n=$((n+1)).",
        ),
    ),
    (
        ShellRule::Sc3040,
        RuleText::new(
            "set -o option outside POSIX in a sh script (ShellCheck SC3040)",
            "drop the option or switch the script to bash",
            BASHISM_WHY,
            "set -o pipefail is not available in every sh: check each command's status instead.",
        ),
    ),
    (
        ShellRule::Sc3041,
        RuleText::new(
            "set flag outside POSIX in a sh script (ShellCheck SC3041)",
            "drop the flag or switch the script to bash",
            BASHISM_WHY,
            "set -E becomes set -e.",
        ),
    ),
    (
        ShellRule::Sc3042,
        RuleText::new(
            "set long --flag in a sh script (ShellCheck SC3042)",
            "use the short flag or -o name",
            BASHISM_WHY,
            "set --posix becomes set -o posix in bash, or nothing in sh.",
        ),
    ),
    (
        ShellRule::Sc3043,
        RuleText::new(
            "local in a sh script (ShellCheck SC3043)",
            "keep the variable unique, or run the function in a subshell",
            BASHISM_WHY,
            "f() { local x=1; } becomes f() ( x=1; ).",
        ),
    ),
    (
        ShellRule::Sc3044,
        RuleText::new(
            "Command that only bash has in a sh script (ShellCheck SC3044)",
            "use a POSIX alternative or switch the script to bash",
            BASHISM_WHY,
            "declare -a list becomes set -- or a plain variable.",
        ),
    ),
    (
        ShellRule::Sc3045,
        RuleText::new(
            "Command flag outside POSIX in a sh script (ShellCheck SC3045)",
            "use only the flags POSIX defines",
            BASHISM_WHY,
            "read -a parts becomes IFS=: read -r first rest.",
        ),
    ),
    (
        ShellRule::Sc3046,
        RuleText::new(
            "source instead of . in a sh script (ShellCheck SC3046)",
            "use .",
            BASHISM_WHY,
            "source ./env.sh becomes . ./env.sh.",
        ),
    ),
    (
        ShellRule::Sc3047,
        RuleText::new(
            "trap on ERR, DEBUG or RETURN in a sh script (ShellCheck SC3047)",
            "trap EXIT or check the status after each command",
            BASHISM_WHY,
            "trap cleanup ERR becomes trap cleanup EXIT.",
        ),
    ),
    (
        ShellRule::Sc3048,
        RuleText::new(
            "Signal name with the SIG prefix in a sh script (ShellCheck SC3048)",
            "drop the SIG prefix",
            BASHISM_WHY,
            "trap f SIGINT becomes trap f INT.",
        ),
    ),
    (
        ShellRule::Sc3049,
        RuleText::new(
            "Signal name in lower or mixed case in a sh script (ShellCheck SC3049)",
            "write the signal name in uppercase",
            BASHISM_WHY,
            "trap f int becomes trap f INT.",
        ),
    ),
    (
        ShellRule::Sc3050,
        RuleText::new(
            "printf %q in a sh script (ShellCheck SC3050)",
            "quote the value yourself, for example with sed",
            BASHISM_WHY,
            "printf '%q' \"$x\" has no POSIX form: wrap the value in single quotes with sed.",
        ),
    ),
    (
        ShellRule::Sc3052,
        RuleText::new(
            "Arithmetic base conversion base#number in a sh script (ShellCheck SC3052)",
            "convert with printf or a decimal number",
            BASHISM_WHY,
            "$((16#ff)) becomes $(printf '%d' 0xff).",
        ),
    ),
    (
        ShellRule::Sc3053,
        RuleText::new(
            "Indirect expansion ${!var} in a sh script (ShellCheck SC3053)",
            "use eval carefully, or restructure the data",
            BASHISM_WHY,
            "${!name} becomes eval \"value=\\${$name}\".",
        ),
    ),
    (
        ShellRule::Sc3054,
        RuleText::new(
            "Array reference ${var[i]} in a sh script (ShellCheck SC3054)",
            "use positional parameters or separate variables",
            BASHISM_WHY,
            "${list[0]} becomes $1 after set -- $list.",
        ),
    ),
    (
        ShellRule::Sc3055,
        RuleText::new(
            "Array key expansion ${!var[@]} in a sh script (ShellCheck SC3055)",
            "keep the keys in their own list",
            BASHISM_WHY,
            "${!list[@]} becomes a separate keys variable.",
        ),
    ),
    (
        ShellRule::Sc3056,
        RuleText::new(
            "Name prefix expansion ${!prefix*} in a sh script (ShellCheck SC3056)",
            "list the variables with set and filter them",
            BASHISM_WHY,
            "${!APP_*} becomes set | grep '^APP_'.",
        ),
    ),
    (
        ShellRule::Sc3057,
        RuleText::new(
            "String indexing ${var:start:length} in a sh script (ShellCheck SC3057)",
            "use cut, expr or a pattern removal",
            BASHISM_WHY,
            "${s:0:3} becomes $(printf '%s' \"$s\" | cut -c1-3).",
        ),
    ),
    (
        ShellRule::Sc3058,
        RuleText::new(
            "String operation on $@ or $* in a sh script (ShellCheck SC3058)",
            "loop over the arguments and apply it to each",
            BASHISM_WHY,
            "${@%.txt} becomes for f; do echo \"${f%.txt}\"; done.",
        ),
    ),
    (
        ShellRule::Sc3059,
        RuleText::new(
            "Case modification ${var^} or ${var,,} in a sh script (ShellCheck SC3059)",
            "use tr",
            BASHISM_WHY,
            "${name^^} becomes $(printf '%s' \"$name\" | tr '[:lower:]' '[:upper:]').",
        ),
    ),
    (
        ShellRule::Sc3060,
        RuleText::new(
            "String replacement ${var/a/b} in a sh script (ShellCheck SC3060)",
            "use sed",
            BASHISM_WHY,
            "${s/a/b} becomes $(printf '%s' \"$s\" | sed 's/a/b/').",
        ),
    ),
    (
        ShellRule::Sc3061,
        RuleText::new(
            "read without a variable in a sh script (ShellCheck SC3061)",
            "name the variable to read into",
            BASHISM_WHY,
            "read -r becomes read -r line.",
        ),
    ),
    (
        ShellRule::Sc3062,
        RuleText::new(
            "test -o to check a shell option in a sh script (ShellCheck SC3062)",
            "check $- for the option's letter",
            BASHISM_WHY,
            "[ -o errexit ] becomes case $- in *e*) ...;; esac.",
        ),
    ),
    (
        ShellRule::Sc3063,
        RuleText::new(
            "test -R and namerefs in a sh script (ShellCheck SC3063)",
            "avoid namerefs and pass the value itself",
            BASHISM_WHY,
            "[ -R ref ] has no POSIX equivalent: restructure the code.",
        ),
    ),
    (
        ShellRule::Sc3064,
        RuleText::new(
            "test -N in a sh script (ShellCheck SC3064)",
            "compare modification times with find -newer",
            BASHISM_WHY,
            "[ -N file ] becomes a find file -newer marker check.",
        ),
    ),
    (
        ShellRule::Sc3065,
        RuleText::new(
            "test -k in a sh script (ShellCheck SC3065)",
            "check the sticky bit with ls -ld or stat",
            BASHISM_WHY,
            "[ -k /tmp ] becomes a check on the t in ls -ld /tmp.",
        ),
    ),
    (
        ShellRule::Sc3066,
        RuleText::new(
            "test -G in a sh script (ShellCheck SC3066)",
            "compare the file's group with stat and id -g",
            BASHISM_WHY,
            "[ -G file ] becomes [ \"$(stat -c %g file)\" = \"$(id -g)\" ].",
        ),
    ),
    (
        ShellRule::Sc3067,
        RuleText::new(
            "test -O in a sh script (ShellCheck SC3067)",
            "compare the file's owner with stat and id -u",
            BASHISM_WHY,
            "[ -O file ] becomes [ \"$(stat -c %u file)\" = \"$(id -u)\" ].",
        ),
    ),
];
