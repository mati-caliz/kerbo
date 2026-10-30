use super::BASHISM_WHY;
use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// The constructs of ShellCheck's checkBashisms.
pub const BASHISMS_TEXTS: [(ShellRule, RuleText); 33] = [
    (
        ShellRule::Sc3001,
        RuleText::new(
            "Process substitution <(...) in a sh script (ShellCheck SC3001)",
            "use a temporary file or a pipe",
            BASHISM_WHY,
            "diff <(a) <(b) becomes a > x; b > y; diff x y.",
        ),
    ),
    (
        ShellRule::Sc3002,
        RuleText::new(
            "extglob pattern in a sh script (ShellCheck SC3002)",
            "use a case statement or several plain globs",
            BASHISM_WHY,
            "rm !(*.hs) becomes a loop that skips *.hs.",
        ),
    ),
    (
        ShellRule::Sc3003,
        RuleText::new(
            "$'...' string in a sh script (ShellCheck SC3003)",
            "use printf to produce the special characters",
            BASHISM_WHY,
            "tab=$'\\t' becomes tab=$(printf '\\t').",
        ),
    ),
    (
        ShellRule::Sc3004,
        RuleText::new(
            "$\"...\" translated string in a sh script (ShellCheck SC3004)",
            "use a plain double-quoted string, or gettext",
            BASHISM_WHY,
            "echo $\"Hello\" becomes echo \"Hello\".",
        ),
    ),
    (
        ShellRule::Sc3005,
        RuleText::new(
            "Arithmetic for loop for ((...)) in a sh script (ShellCheck SC3005)",
            "use a while loop with a counter",
            BASHISM_WHY,
            "for ((i=0; i<3; i++)) becomes i=0; while [ $i -lt 3 ]; do ...; i=$((i+1)); done.",
        ),
    ),
    (
        ShellRule::Sc3006,
        RuleText::new(
            "Standalone ((...)) arithmetic in a sh script (ShellCheck SC3006)",
            "use [ $((...)) -ne 0 ] or an assignment with $((...))",
            BASHISM_WHY,
            "((n++)) becomes n=$((n+1)).",
        ),
    ),
    (
        ShellRule::Sc3007,
        RuleText::new(
            "$[...] arithmetic in a sh script (ShellCheck SC3007)",
            "use $((...))",
            BASHISM_WHY,
            "$[a+1] becomes $((a+1)).",
        ),
    ),
    (
        ShellRule::Sc3008,
        RuleText::new(
            "select loop in a sh script (ShellCheck SC3008)",
            "print the options and read the choice with read",
            BASHISM_WHY,
            "select x in a b becomes a numbered list and read choice.",
        ),
    ),
    (
        ShellRule::Sc3009,
        RuleText::new(
            "Brace expansion {a,b} or {1..3} in a sh script (ShellCheck SC3009)",
            "write the words out or use a loop",
            BASHISM_WHY,
            "cp f{,.bak} becomes cp f f.bak.",
        ),
    ),
    (
        ShellRule::Sc3010,
        RuleText::new(
            "[[ ]] test in a sh script (ShellCheck SC3010)",
            "use [ ] with quoted variables, or case for patterns",
            BASHISM_WHY,
            "[[ $a == b* ]] becomes case $a in b*) ...;; esac.",
        ),
    ),
    (
        ShellRule::Sc3011,
        RuleText::new(
            "Here-string <<< in a sh script (ShellCheck SC3011)",
            "use a pipe from printf or a here-document",
            BASHISM_WHY,
            "grep x <<< \"$s\" becomes printf '%s\\n' \"$s\" | grep x.",
        ),
    ),
    (
        ShellRule::Sc3012,
        RuleText::new(
            "Lexicographic < or > in a test in a sh script (ShellCheck SC3012)",
            "compare with expr, or sort the two values",
            BASHISM_WHY,
            "[ \"$a\" \\< \"$b\" ] becomes [ \"$(printf '%s\\n' \"$a\" \"$b\" | sort | head -n1)\" = \"$a\" ].",
        ),
    ),
    (
        ShellRule::Sc3014,
        RuleText::new(
            "== in a test instead of = in a sh script (ShellCheck SC3014)",
            "use a single =",
            BASHISM_WHY,
            "[ \"$a\" == b ] becomes [ \"$a\" = b ].",
        ),
    ),
    (
        ShellRule::Sc3015,
        RuleText::new(
            "=~ regex matching in a sh script (ShellCheck SC3015)",
            "use grep -E, expr or case",
            BASHISM_WHY,
            "[[ $a =~ ^[0-9]+$ ]] becomes printf '%s' \"$a\" | grep -Eq '^[0-9]+$'.",
        ),
    ),
    (
        ShellRule::Sc3016,
        RuleText::new(
            "test -v in a sh script (ShellCheck SC3016)",
            "use [ -n \"${var+x}\" ]",
            BASHISM_WHY,
            "[ -v name ] becomes [ -n \"${name+x}\" ].",
        ),
    ),
    (
        ShellRule::Sc3017,
        RuleText::new(
            "Unary -a in a test in a sh script (ShellCheck SC3017)",
            "use -e",
            BASHISM_WHY,
            "[ -a file ] becomes [ -e file ].",
        ),
    ),
    (
        ShellRule::Sc3018,
        RuleText::new(
            "++ or -- in arithmetic in a sh script (ShellCheck SC3018)",
            "use an explicit + 1 or - 1",
            BASHISM_WHY,
            "$((i++)) becomes i=$((i+1)).",
        ),
    ),
    (
        ShellRule::Sc3019,
        RuleText::new(
            "** exponent in arithmetic in a sh script (ShellCheck SC3019)",
            "multiply, or compute it with awk or bc",
            BASHISM_WHY,
            "$((2**8)) becomes 256 or $(echo '2^8' | bc).",
        ),
    ),
    (
        ShellRule::Sc3020,
        RuleText::new(
            "&> redirection in a sh script (ShellCheck SC3020)",
            "redirect stdout and then stderr: >file 2>&1",
            BASHISM_WHY,
            "cmd &> log becomes cmd > log 2>&1.",
        ),
    ),
    (
        ShellRule::Sc3021,
        RuleText::new(
            ">& to a file name in a sh script (ShellCheck SC3021)",
            "use >file 2>&1",
            BASHISM_WHY,
            "cmd >& log becomes cmd > log 2>&1.",
        ),
    ),
    (
        ShellRule::Sc3022,
        RuleText::new(
            "Named file descriptor {fd}> in a sh script (ShellCheck SC3022)",
            "pick a fixed descriptor number between 3 and 9",
            BASHISM_WHY,
            "exec {fd}> log becomes exec 3> log.",
        ),
    ),
    (
        ShellRule::Sc3023,
        RuleText::new(
            "File descriptor above 9 in a sh script (ShellCheck SC3023)",
            "use a descriptor between 0 and 9",
            BASHISM_WHY,
            "exec 10> log becomes exec 9> log.",
        ),
    ),
    (
        ShellRule::Sc3024,
        RuleText::new(
            "+= assignment in a sh script (ShellCheck SC3024)",
            "concatenate explicitly",
            BASHISM_WHY,
            "path+=:/opt becomes path=\"$path:/opt\".",
        ),
    ),
    (
        ShellRule::Sc3025,
        RuleText::new(
            "/dev/tcp or /dev/udp redirection in a sh script (ShellCheck SC3025)",
            "use nc, curl or another tool",
            BASHISM_WHY,
            "cat < /dev/tcp/host/80 becomes nc host 80.",
        ),
    ),
    (
        ShellRule::Sc3026,
        RuleText::new(
            "^ to negate a glob bracket expression in a sh script (ShellCheck SC3026)",
            "use ! instead of ^",
            BASHISM_WHY,
            "ls [^a]* becomes ls [!a]*.",
        ),
    ),
    (
        ShellRule::Sc3028,
        RuleText::new(
            "Variable that only bash sets in a sh script (ShellCheck SC3028)",
            "compute the value another way, or switch the script to bash",
            BASHISM_WHY,
            "echo $RANDOM becomes awk 'BEGIN{srand(); print int(rand()*32768)}'.",
        ),
    ),
    (
        ShellRule::Sc3029,
        RuleText::new(
            "|& pipe in a sh script (ShellCheck SC3029)",
            "use 2>&1 |",
            BASHISM_WHY,
            "cmd |& tee log becomes cmd 2>&1 | tee log.",
        ),
    ),
    (
        ShellRule::Sc3030,
        RuleText::new(
            "Array in a sh script (ShellCheck SC3030)",
            "use positional parameters with set --, or a delimited string",
            BASHISM_WHY,
            "files=(a b) becomes set -- a b.",
        ),
    ),
    (
        ShellRule::Sc3031,
        RuleText::new(
            "Redirection to or from a glob in a sh script (ShellCheck SC3031)",
            "write the file name out, or loop over the matches",
            BASHISM_WHY,
            "cat > out*.log becomes cat > out.log.",
        ),
    ),
    (
        ShellRule::Sc3032,
        RuleText::new(
            "coproc in a sh script (ShellCheck SC3032)",
            "use a named pipe (mkfifo) and a background job",
            BASHISM_WHY,
            "coproc worker becomes mkfifo fifo; worker < fifo &.",
        ),
    ),
    (
        ShellRule::Sc3033,
        RuleText::new(
            "Function name outside [a-zA-Z_][a-zA-Z0-9_]* in a sh script (ShellCheck SC3033)",
            "rename the function with letters, digits and underscores",
            BASHISM_WHY,
            "foo-bar() becomes foo_bar().",
        ),
    ),
    (
        ShellRule::Sc3034,
        RuleText::new(
            "$(<file) to read a file in a sh script (ShellCheck SC3034)",
            "use $(cat file)",
            BASHISM_WHY,
            "text=$(<file) becomes text=$(cat file).",
        ),
    ),
    (
        ShellRule::Sc3035,
        RuleText::new(
            "`<file` to read a file in a sh script (ShellCheck SC3035)",
            "use $(cat file)",
            BASHISM_WHY,
            "text=`<file` becomes text=$(cat file).",
        ),
    ),
];
