use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// The checks of ShellCheck's Checks/Commands.hs on external programs.
pub const TOOLS_TEXTS: [(ShellRule, RuleText); 37] = [
    (
        ShellRule::Sc2003,
        RuleText::new(
            "expr instead of the shell's own arithmetic (ShellCheck SC2003)",
            "use $((...)), ${...} or [ ]",
            "expr is an external program with an awkward syntax; the shell already does arithmetic and string handling.",
            "n=$(expr $n + 1) becomes n=$((n + 1)).",
        ),
    ),
    (
        ShellRule::Sc2018,
        RuleText::new(
            "tr range a-z that misses accented letters (ShellCheck SC2018)",
            "use '[:lower:]'",
            "a-z only covers the ASCII letters, so accented and foreign letters are left as they are.",
            "tr a-z A-Z becomes tr '[:lower:]' '[:upper:]'.",
        ),
    ),
    (
        ShellRule::Sc2019,
        RuleText::new(
            "tr range A-Z that misses accented letters (ShellCheck SC2019)",
            "use '[:upper:]'",
            "A-Z only covers the ASCII letters, so accented and foreign letters are left as they are.",
            "tr A-Z a-z becomes tr '[:upper:]' '[:lower:]'.",
        ),
    ),
    (
        ShellRule::Sc2020,
        RuleText::new(
            "tr set with repeated letters, as if it replaced words (ShellCheck SC2020)",
            "use sed to replace words; tr maps characters one by one",
            "tr replaces each character of the first set by the one in the same place of the second, it does not replace strings.",
            "tr 'hello' 'world' becomes sed 's/hello/world/g'.",
        ),
    ),
    (
        ShellRule::Sc2021,
        RuleText::new(
            "[] around a tr class, which also replaces the brackets (ShellCheck SC2021)",
            "drop the brackets",
            "tr does not use brackets for ranges: '[a-z]' also maps the literal [ and ] characters.",
            "tr '[a-z]' '[A-Z]' becomes tr 'a-z' 'A-Z'.",
        ),
    ),
    (
        ShellRule::Sc2022,
        RuleText::new(
            "x* in a grep regex read as a glob (ShellCheck SC2022)",
            "write x.* if you meant x followed by anything",
            "In a regex x* means zero or more x, not x followed by anything as in a glob.",
            "grep 'foo*' becomes grep 'foo.*'.",
        ),
    ),
    (
        ShellRule::Sc2023,
        RuleText::new(
            "time with flags, which the shell keyword ignores (ShellCheck SC2023)",
            "use command time to run the time program",
            "In bash and sh time is a keyword with no flags; the -f or -v of man time(1) belong to /usr/bin/time.",
            "time -v cmd becomes command time -v cmd.",
        ),
    ),
    (
        ShellRule::Sc2024,
        RuleText::new(
            "Redirection on a sudo command, which runs without sudo (ShellCheck SC2024)",
            "use sudo tee or sudo cat for the file",
            "The shell opens the redirected file before running sudo, with the current user's permissions.",
            "sudo echo x > /etc/f becomes echo x | sudo tee /etc/f.",
        ),
    ),
    (
        ShellRule::Sc2028,
        RuleText::new(
            "echo with escape sequences it may not expand (ShellCheck SC2028)",
            "use printf",
            "Whether echo expands \\n or \\t depends on the shell and its options; printf does it the same everywhere.",
            "echo \"a\\tb\" becomes printf 'a\\tb\\n'.",
        ),
    ),
    (
        ShellRule::Sc2029,
        RuleText::new(
            "Expansion in an ssh command that runs on the client (ShellCheck SC2029)",
            "escape the $ or use single quotes if it should expand on the server",
            "The local shell expands the double-quoted command before ssh sends it.",
            "ssh host \"echo $HOME\" shows the local HOME; ssh host 'echo $HOME' the remote one.",
        ),
    ),
    (
        ShellRule::Sc2060,
        RuleText::new(
            "Unquoted tr argument that the shell can expand as a glob (ShellCheck SC2060)",
            "quote the tr sets",
            "Unquoted, [a-z] can match a file name in the current directory and tr receives that instead.",
            "tr [a-z] [A-Z] becomes tr '[:lower:]' '[:upper:]'.",
        ),
    ),
    (
        ShellRule::Sc2061,
        RuleText::new(
            "Unquoted find pattern that the shell expands first (ShellCheck SC2061)",
            "quote the pattern of -name, -path and the like",
            "If a file in the current directory matches, find receives its name instead of the pattern.",
            "find . -name *.log becomes find . -name '*.log'.",
        ),
    ),
    (
        ShellRule::Sc2062,
        RuleText::new(
            "Unquoted grep pattern that the shell can expand (ShellCheck SC2062)",
            "quote the pattern",
            "Unquoted, the pattern can match file names and grep receives those instead.",
            "grep foo* file becomes grep 'foo*' file.",
        ),
    ),
    (
        ShellRule::Sc2063,
        RuleText::new(
            "grep pattern that looks like a glob (ShellCheck SC2063)",
            "write it as a regex, or use grep -F for fixed text",
            "grep reads a regex: *.txt means any repetition of nothing followed by any character and txt.",
            "grep '*.txt' becomes grep '\\.txt$'.",
        ),
    ),
    (
        ShellRule::Sc2114,
        RuleText::new(
            "rm -r on a system directory (ShellCheck SC2114)",
            "check the path: this deletes a system directory",
            "A recursive rm of / or /usr wipes the machine.",
            "rm -rf /usr/ becomes rm -rf ./usr/.",
        ),
    ),
    (
        ShellRule::Sc2115,
        RuleText::new(
            "rm -r on a path that becomes a system directory if the variable is empty (ShellCheck SC2115)",
            "use \"${var:?}\" so it fails when empty",
            "If the variable is empty or unset, rm -rf \"$dir/\" deletes the root directory.",
            "rm -rf \"$dir/\" becomes rm -rf \"${dir:?}/\".",
        ),
    ),
    (
        ShellRule::Sc2117,
        RuleText::new(
            "su without a command, which opens an interactive shell (ShellCheck SC2117)",
            "use su -c 'command' or sudo",
            "The rest of the script does not run as the other user: it waits until that shell ends.",
            "su postgres; psql becomes su postgres -c psql.",
        ),
    ),
    (
        ShellRule::Sc2146,
        RuleText::new(
            "find action that ignores the tests before -o (ShellCheck SC2146)",
            "group the tests with \\( \\)",
            "-o binds looser than the implicit -a, so the action only applies to the last test.",
            "find . -name a -o -name b -delete becomes find . \\( -name a -o -name b \\) -delete.",
        ),
    ),
    (
        ShellRule::Sc2150,
        RuleText::new(
            "find -exec with a shell command in one argument (ShellCheck SC2150)",
            "use -exec sh -c '...' or split the command into arguments",
            "-exec runs a program, not a shell: a pipe or ; inside one argument is part of the program name.",
            "find . -exec 'grep x | wc' {} \\; becomes find . -exec sh -c 'grep x \"$1\" | wc' _ {} \\;.",
        ),
    ),
    (
        ShellRule::Sc2156,
        RuleText::new(
            "{} inside the script of find -exec sh -c (ShellCheck SC2156)",
            "pass the file as a parameter: sh -c '... \"$1\"' _ {}",
            "A file name with quotes or $ becomes code inside the shell script.",
            "find . -exec sh -c 'cat {}' \\; becomes find . -exec sh -c 'cat \"$1\"' _ {} \\;.",
        ),
    ),
    (
        ShellRule::Sc2174,
        RuleText::new(
            "mkdir -p -m, where the mode only applies to the last directory (ShellCheck SC2174)",
            "create the directories and run chmod on each, or set umask",
            "The parent directories mkdir -p creates get the default mode, not the one passed with -m.",
            "mkdir -p -m 700 a/b creates a with the default mode.",
        ),
    ),
    (
        ShellRule::Sc2185,
        RuleText::new(
            "find without a path (ShellCheck SC2185)",
            "pass . explicitly",
            "Some versions of find have no default path and fail.",
            "find -name x becomes find . -name x.",
        ),
    ),
    (
        ShellRule::Sc2186,
        RuleText::new(
            "tempfile, which is deprecated (ShellCheck SC2186)",
            "use mktemp",
            "tempfile was removed from Debian and other systems; mktemp is the standard way.",
            "tmp=$(tempfile) becomes tmp=$(mktemp).",
        ),
    ),
    (
        ShellRule::Sc2196,
        RuleText::new(
            "egrep, which is deprecated (ShellCheck SC2196)",
            "use grep -E",
            "egrep is not standard and recent grep versions warn every time it runs.",
            "egrep 'a|b' becomes grep -E 'a|b'.",
        ),
    ),
    (
        ShellRule::Sc2197,
        RuleText::new(
            "fgrep, which is deprecated (ShellCheck SC2197)",
            "use grep -F",
            "fgrep is not standard and recent grep versions warn every time it runs.",
            "fgrep 'a.b' becomes grep -F 'a.b'.",
        ),
    ),
    (
        ShellRule::Sc2224,
        RuleText::new(
            "mv without a destination (ShellCheck SC2224)",
            "add the destination",
            "With a single argument mv fails, which usually means one argument was lost.",
            "mv \"$file\" becomes mv \"$file\" \"$destination\".",
        ),
    ),
    (
        ShellRule::Sc2225,
        RuleText::new(
            "cp without a destination (ShellCheck SC2225)",
            "add the destination",
            "With a single argument cp fails, which usually means one argument was lost.",
            "cp \"$file\" becomes cp \"$file\" \"$destination\".",
        ),
    ),
    (
        ShellRule::Sc2226,
        RuleText::new(
            "ln without a destination (ShellCheck SC2226)",
            "add the destination, or . explicitly",
            "With a single argument ln links into the current directory, which is rarely what was meant.",
            "ln -s \"$target\" becomes ln -s \"$target\" \"$link\".",
        ),
    ),
    (
        ShellRule::Sc2227,
        RuleText::new(
            "Redirection in the middle of find's arguments (ShellCheck SC2227)",
            "move the redirection to the end, or redirect inside each action",
            "The redirection applies to find itself, not to the action it is next to.",
            "find . -print > a -exec x {} \\; becomes find . -exec x {} \\; > a.",
        ),
    ),
    (
        ShellRule::Sc2232,
        RuleText::new(
            "sudo with a shell builtin (ShellCheck SC2232)",
            "use sudo sh -c '...'",
            "Builtins like cd or export are not programs, so sudo cannot find them.",
            "sudo cd /root becomes sudo sh -c 'cd /root && ...'.",
        ),
    ),
    (
        ShellRule::Sc2253,
        RuleText::new(
            "chmod -r, which removes read permission (ShellCheck SC2253)",
            "use -R to recurse, or a-r if removing read permission is the intent",
            "-r is the mode 'remove read', not the recursive flag.",
            "chmod -r 755 dir becomes chmod -R 755 dir.",
        ),
    ),
    (
        ShellRule::Sc2267,
        RuleText::new(
            "xargs -i, which is deprecated (ShellCheck SC2267)",
            "use -I{}",
            "GNU xargs deprecated -i in favor of -I, which works in every xargs.",
            "xargs -i cp {} dst becomes xargs -I{} cp {} dst.",
        ),
    ),
    (
        ShellRule::Sc2304,
        RuleText::new(
            "Unescaped * in expr multiplication (ShellCheck SC2304)",
            "escape it as \\* or use $((x * y))",
            "Unquoted, * expands to the files in the current directory.",
            "expr 2 * 3 becomes echo $((2 * 3)).",
        ),
    ),
    (
        ShellRule::Sc2305,
        RuleText::new(
            "Unquoted expr regex that the shell can expand (ShellCheck SC2305)",
            "quote the regex",
            "Unquoted, the regex can match file names and expr receives those instead.",
            "expr \"$a\" : .*x becomes expr \"$a\" : '.*x'.",
        ),
    ),
    (
        ShellRule::Sc2306,
        RuleText::new(
            "Unescaped glob character in an expr argument (ShellCheck SC2306)",
            "escape or quote it",
            "Unquoted, the glob can expand to file names before expr sees it.",
            "expr 2 * 3 + 1 becomes expr 2 \\* 3 + 1.",
        ),
    ),
    (
        ShellRule::Sc2307,
        RuleText::new(
            "expr with fewer than three arguments (ShellCheck SC2307)",
            "separate each operand and operator into its own argument",
            "expr needs the operands and the operator as separate words; '1+1' is just a string.",
            "expr 1+1 becomes expr 1 + 1.",
        ),
    ),
    (
        ShellRule::Sc2308,
        RuleText::new(
            "expr match, length, substr or index, which are not standard (ShellCheck SC2308)",
            "use expr str : regex, ${#var} or cut",
            "These forms are GNU extensions with unspecified results elsewhere.",
            "expr length \"$s\" becomes ${#s}.",
        ),
    ),
];
