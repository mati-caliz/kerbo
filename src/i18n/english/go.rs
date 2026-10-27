use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;

const UNKNOWN: RuleText = RuleText::new(
    "golangci-lint rule",
    "follow the rule's advice",
    "golangci-lint reports it by default.",
    "See the golangci-lint documentation.",
);
const TEXTS: [RuleText; GoRule::ALL.len()] = [
    RuleText::new(
        "Assignment nobody reads before it is overwritten or the function ends",
        "drop the assignment or use the value before assigning again",
        "The value is computed and thrown away: either the computation is extra or a use is missing, and the latter is usually a lost bug, like an err nobody checks.",
        "x := load(); x = other() becomes x := other().",
    ),
    RuleText::new(
        "Error returned by a call that nobody checks (errcheck)",
        "check the error, return it or, if it truly does not matter, assign it to _ with a comment on why",
        "An ignored error carries on as if everything went fine: a file that was not closed, data that was not saved, a connection that dropped.",
        "f.Close() becomes if err := f.Close(); err != nil { return err }.",
    ),
    RuleText::new(
        "Error message that starts with a capital letter or ends with punctuation (staticcheck ST1005)",
        "start the message in lowercase and drop the trailing period, colon, mark or newline",
        "Errors get wrapped: fmt.Errorf(\"save: %w\", err) builds one sentence, and a capital letter or a period in the middle breaks it.",
        "errors.New(\"Order not found.\") becomes errors.New(\"order not found\").",
    ),
    RuleText::new(
        "if and else if chain that always compares the same thing with == (staticcheck QF1003)",
        "turn it into a switch on the compared value",
        "A switch says up front that every branch looks at the same value; the if chain makes you read each condition to notice.",
        "if x == 1 { a() } else if x == 2 { b() } becomes switch x { case 1: a(); case 2: b() }.",
    ),
    RuleText::new(
        "strings.Replace, strings.SplitN or their bytes twins with -1 as the limit (staticcheck QF1004)",
        "use strings.ReplaceAll, strings.Split or strings.SplitAfter without the -1",
        "The -1 only means \"no limit\": the unlimited function says it in its name and leaves no magic number.",
        "strings.Replace(s, \"a\", \"b\", -1) becomes strings.ReplaceAll(s, \"a\", \"b\").",
    ),
];

pub fn go(rule: GoRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
