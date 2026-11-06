use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// ShellCheck's checks that follow the kind of each variable and the control flow graph.
pub const FLOW_KINDS_TEXTS: [(ShellRule, RuleText); 10] = [
    (
        ShellRule::Sc2089,
        RuleText::new(
            "Quotes or backslashes stored in a variable (ShellCheck SC2089)",
            "use an array, or functions in sh",
            "They are treated literally when the variable is expanded.",
            "a='--x=\"b c\"' becomes a=(--x=\"b c\").",
        ),
    ),
    (
        ShellRule::Sc2090,
        RuleText::new(
            "Expansion of a variable with stored quotes (ShellCheck SC2090)",
            "use an array, or functions in sh",
            "The quotes inside the value are not respected.",
            "cmd $a becomes cmd \"${a[@]}\".",
        ),
    ),
    (
        ShellRule::Sc2128,
        RuleText::new(
            "Array expanded without an index (ShellCheck SC2128)",
            "use \"${a[@]}\" or an index",
            "It only gives the first element.",
            "echo $a becomes echo \"${a[@]}\".",
        ),
    ),
    (
        ShellRule::Sc2178,
        RuleText::new(
            "Array assigned a string (ShellCheck SC2178)",
            "use another variable, or a=(value)",
            "It only replaces the first element.",
            "a=(1 2); a=x becomes a=(x).",
        ),
    ),
    (
        ShellRule::Sc2179,
        RuleText::new(
            "+= with a string on an array (ShellCheck SC2179)",
            "use a+=(\"item\")",
            "It appends the text to the first element.",
            "a=(1); a+=x becomes a+=(x).",
        ),
    ),
    (
        ShellRule::Sc2218,
        RuleText::new(
            "Function only defined later (ShellCheck SC2218)",
            "move the definition up",
            "When the call runs, the function does not exist yet.",
            "f; f() { :; } becomes f() { :; }; f.",
        ),
    ),
    (
        ShellRule::Sc2317,
        RuleText::new(
            "Command that can never run (ShellCheck SC2317)",
            "remove it, or fix the flow before it",
            "Something before it always ends the script or the function.",
            "exit; foo becomes exit.",
        ),
    ),
    (
        ShellRule::Sc2319,
        RuleText::new(
            "$? that refers to a condition (ShellCheck SC2319)",
            "save it in a variable before the condition",
            "The test overwrote the code of the previous command.",
            "[ $? -eq 0 ] && a=$? needs r=$? first.",
        ),
    ),
    (
        ShellRule::Sc2320,
        RuleText::new(
            "$? that refers to echo or printf (ShellCheck SC2320)",
            "save it in a variable before printing",
            "The print overwrote the code of the previous command.",
            "echo a; [ $? -eq 0 ] needs r=$? first.",
        ),
    ),
    (
        ShellRule::Sc2329,
        RuleText::new(
            "Function never invoked (ShellCheck SC2329)",
            "remove it, or check whether it is called indirectly",
            "Nothing in the script reaches it.",
            "f() { :; }; exit has no call to f.",
        ),
    ),
];
