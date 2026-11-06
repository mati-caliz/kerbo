use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// ShellCheck's Analytics.hs checks on the shape of a command: names that cannot be commands, pipes and redirections that lead nowhere, and badly written array elements.
pub const COMMAND_SHAPES_TEXTS: [(ShellRule, RuleText); 36] = [
    (
        ShellRule::Sc2188,
        RuleText::new(
            "Redirection without a command (ShellCheck SC2188)",
            "move it to its command, or use true as a no-op",
            "Nothing runs, and the file is created or emptied.",
            "> file becomes true > file.",
        ),
    ),
    (
        ShellRule::Sc2189,
        RuleText::new(
            "| between a redirection and its command (ShellCheck SC2189)",
            "put the redirection next to the command",
            "The redirection applies to nothing, and the command reads an empty pipe.",
            "< file | cmd becomes cmd < file.",
        ),
    ),
    (
        ShellRule::Sc2190,
        RuleText::new(
            "Associative array element without an index (ShellCheck SC2190)",
            "use ( [index]=value )",
            "Associative arrays have no positions: the element is lost.",
            "declare -A a=(b) becomes declare -A a=([k]=b).",
        ),
    ),
    (
        ShellRule::Sc2191,
        RuleText::new(
            "Literal = in an array element (ShellCheck SC2191)",
            "use ( [index]=value ), or quote it",
            "Without brackets the = stays in the value: there is no index.",
            "a=(1=b) becomes a=([1]=b).",
        ),
    ),
    (
        ShellRule::Sc2192,
        RuleText::new(
            "Array element without a value (ShellCheck SC2192)",
            "remove the spaces after =, or use \"\"",
            "The next word is a separate element.",
            "a=([x]= b) becomes a=([x]=b).",
        ),
    ),
    (
        ShellRule::Sc2210,
        RuleText::new(
            "Redirection to a file named like a number (ShellCheck SC2210)",
            "use -gt for a comparison, or >&2 for a descriptor",
            "It creates a file called 2.",
            "foo > 2 becomes foo >&2.",
        ),
    ),
    (
        ShellRule::Sc2211,
        RuleText::new(
            "Glob used as a command name (ShellCheck SC2211)",
            "put it in an array or ${..}, or quote it",
            "The first matching file would run as a command.",
            "*.sh becomes ./run.sh.",
        ),
    ),
    (
        ShellRule::Sc2215,
        RuleText::new(
            "Flag used as a command name (ShellCheck SC2215)",
            "fix the line break, or add the missing [ .. ]",
            "There is no command called -e.",
            "-e file becomes [ -e file ].",
        ),
    ),
    (
        ShellRule::Sc2216,
        RuleText::new(
            "Pipe to a command that does not read stdin (ShellCheck SC2216)",
            "use cat or xargs",
            "The command ignores what comes through the pipe.",
            "foo | rm becomes foo | xargs rm.",
        ),
    ),
    (
        ShellRule::Sc2217,
        RuleText::new(
            "Redirection to a command that does not read stdin (ShellCheck SC2217)",
            "use cat or xargs",
            "The command ignores that input.",
            "echo < file becomes cat file.",
        ),
    ),
    (
        ShellRule::Sc2238,
        RuleText::new(
            "Redirection to or from a command name (ShellCheck SC2238)",
            "use a pipe or xargs, or quote it",
            "It is treated as a file with that name.",
            "foo > grep becomes foo | grep.",
        ),
    ),
    (
        ShellRule::Sc2247,
        RuleText::new(
            "$\"(..)\" with $ and the quote swapped (ShellCheck SC2247)",
            "use \"$(..)\"",
            "$\"..\" is a translated string: nothing inside runs.",
            "$\"(ls)\" becomes \"$(ls)\".",
        ),
    ),
    (
        ShellRule::Sc2256,
        RuleText::new(
            "$\"name\" where name is a variable (ShellCheck SC2256)",
            "use \"$name\"",
            "$\"..\" is a translated string, not an expansion.",
            "$\"a\" becomes \"$a\".",
        ),
    ),
    (
        ShellRule::Sc2259,
        RuleText::new(
            "Redirection that overrides the piped input (ShellCheck SC2259)",
            "merge them, or pass file names",
            "The command reads the file and ignores the pipe.",
            "a | b < f needs cat - f | b.",
        ),
    ),
    (
        ShellRule::Sc2260,
        RuleText::new(
            "Redirection that overrides the output pipe (ShellCheck SC2260)",
            "use tee to send it to both",
            "The next stage in the pipeline gets nothing.",
            "a > f | b becomes a | tee f | b.",
        ),
    ),
    (
        ShellRule::Sc2261,
        RuleText::new(
            "Several redirections compete for the same descriptor (ShellCheck SC2261)",
            "use cat, tee, or pass file names",
            "Only the last one takes effect.",
            "a > f > g becomes a | tee f > g.",
        ),
    ),
    (
        ShellRule::Sc2262,
        RuleText::new(
            "Alias defined and used in the same parsing unit (ShellCheck SC2262)",
            "use a function",
            "The shell reads the whole unit before defining the alias.",
            "alias a=b; a becomes a() { b; }; a.",
        ),
    ),
    (
        ShellRule::Sc2263,
        RuleText::new(
            "Command that will not use the alias just defined (ShellCheck SC2263)",
            "use a function",
            "The alias only exists from the next parsing unit on.",
            "alias a=b; a becomes a() { b; }; a.",
        ),
    ),
    (
        ShellRule::Sc2264,
        RuleText::new(
            "Function that always calls itself (ShellCheck SC2264)",
            "add command before the name",
            "Without it the function never ends.",
            "ls() { ls -l; } becomes ls() { command ls -l; }.",
        ),
    ),
    (
        ShellRule::Sc2270,
        RuleText::new(
            "Assignment to a positional parameter (ShellCheck SC2270)",
            "use set -- first second, or [ ] to compare",
            "The shell looks for a command with that name.",
            "1=a becomes set -- a.",
        ),
    ),
    (
        ShellRule::Sc2271,
        RuleText::new(
            "Assignment to a computed name (ShellCheck SC2271)",
            "use arrays, declare \"var$n=value\" or read/eval",
            "The shell looks for a command with that name.",
            "var$n=1 becomes declare \"var$n=1\".",
        ),
    ),
    (
        ShellRule::Sc2272,
        RuleText::new(
            "Command name that contains == (ShellCheck SC2272)",
            "use [ \"$var\" = value ]",
            "The shell looks for a command with that name.",
            "$a==1 becomes [ \"$a\" = 1 ].",
        ),
    ),
    (
        ShellRule::Sc2273,
        RuleText::new(
            "Line of ===== (ShellCheck SC2273)",
            "solve the merge conflict, or make it a comment",
            "The shell tries to run it as a command.",
            "======= becomes # =======.",
        ),
    ),
    (
        ShellRule::Sc2274,
        RuleText::new(
            "Command name that starts with === (ShellCheck SC2274)",
            "make the border a comment",
            "The shell tries to run it as a command.",
            "=== Title === becomes # === Title ===.",
        ),
    ),
    (
        ShellRule::Sc2275,
        RuleText::new(
            "Command name that starts with = (ShellCheck SC2275)",
            "fix the line break",
            "The value got separated from the variable.",
            "a\n=1 becomes a=1.",
        ),
    ),
    (
        ShellRule::Sc2276,
        RuleText::new(
            "Command name that contains = (ShellCheck SC2276)",
            "fix the assignment or the comparison",
            "The shell looks for a command with that name.",
            "\"a\"=1 becomes a=1.",
        ),
    ),
    (
        ShellRule::Sc2277,
        RuleText::new(
            "Assignment to $0 in bash (ShellCheck SC2277)",
            "use BASH_ARGV0",
            "This form looks for a command with that name.",
            "0=a becomes BASH_ARGV0=a.",
        ),
    ),
    (
        ShellRule::Sc2278,
        RuleText::new(
            "Assignment to $0 in ksh (ShellCheck SC2278)",
            "use another variable",
            "$0 cannot be assigned in ksh.",
            "0=a uses another variable.",
        ),
    ),
    (
        ShellRule::Sc2279,
        RuleText::new(
            "Assignment to $0 in dash or BusyBox (ShellCheck SC2279)",
            "use another variable",
            "It becomes a command name.",
            "0=a uses another variable.",
        ),
    ),
    (
        ShellRule::Sc2280,
        RuleText::new(
            "Assignment to $0 in sh (ShellCheck SC2280)",
            "use another variable",
            "There is no portable way to do it.",
            "0=a uses another variable.",
        ),
    ),
    (
        ShellRule::Sc2281,
        RuleText::new(
            "$ on the left side of an assignment (ShellCheck SC2281)",
            "remove the $",
            "It expands the variable and looks for a command with that name.",
            "$a=1 becomes a=1.",
        ),
    ),
    (
        ShellRule::Sc2282,
        RuleText::new(
            "Variable name that starts with a number (ShellCheck SC2282)",
            "use a name that starts with a letter",
            "It is read as a command.",
            "9a=1 becomes a9=1.",
        ),
    ),
    (
        ShellRule::Sc2286,
        RuleText::new(
            "Empty string as a command name (ShellCheck SC2286)",
            "fix the syntax, or use true as a no-op",
            "The shell cannot run an empty name.",
            "\"\" a becomes true.",
        ),
    ),
    (
        ShellRule::Sc2287,
        RuleText::new(
            "Command name that ends in / (ShellCheck SC2287)",
            "fix the syntax",
            "It is a directory, not a program.",
            "foo/ becomes foo.",
        ),
    ),
    (
        ShellRule::Sc2288,
        RuleText::new(
            "Command name that ends in a symbol (ShellCheck SC2288)",
            "fix the syntax",
            "Those symbols are usually a typo or a missing space.",
            "foo, becomes foo.",
        ),
    ),
    (
        ShellRule::Sc2289,
        RuleText::new(
            "Command name with a tab or a line break (ShellCheck SC2289)",
            "fix the syntax",
            "A quote probably got out of place.",
            "\"foo\nbar\" becomes foo.",
        ),
    ),
];
