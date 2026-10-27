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
    RuleText::new(
        "Embedded field that is not needed in a selector chain (staticcheck QF1008)",
        "drop the embedded field: the selector reaches the same field or method without naming it",
        "Naming the embedded field makes the expression longer without saying anything new and ties the code to how the type is built inside.",
        "order.Base.ID becomes order.ID.",
    ),
    RuleText::new(
        "Writing a separately built fmt.Sprintf into an io.Writer (staticcheck QF1012)",
        "use fmt.Fprintf (or Fprint, Fprintln) on the destination",
        "Fprintf writes straight into the destination; Sprintf builds an intermediate string that is thrown away right after.",
        "w.Write([]byte(fmt.Sprintf(\"%d\", n))) becomes fmt.Fprintf(w, \"%d\", n).",
    ),
    RuleText::new(
        "Use of something marked as deprecated (staticcheck SA1019)",
        "move to what the Deprecated: comment of the used symbol recommends",
        "Deprecated things keep working until one day they are gone or stop getting fixes; the comment says what to use instead.",
        "ioutil.ReadFile(path) becomes os.ReadFile(path).",
    ),
    RuleText::new(
        "Negation of a whole comparison that reads better with De Morgan's law (staticcheck QF1001)",
        "invert each comparison and swap && for || (or the other way around) instead of negating the parentheses",
        "A negated parenthesis forces you to invert everything in your head; already inverted comparisons read at once.",
        "!(a == b && c > 0) becomes a != b || c <= 0.",
    ),
    RuleText::new(
        "Method with the name of a well-known interface but another signature (govet stdmethods)",
        "give it the signature of the standard library interface or rename it",
        "With that name, readers (and fmt, encoding/json or io) expect the known interface; with another signature it does not implement it and nobody notices.",
        "func (t T) Seek(off int64) int64 becomes func (t T) Seek(off int64, whence int) (int64, error).",
    ),
    RuleText::new(
        "Package error variable not named errFoo (staticcheck ST1012)",
        "call it errSomething, or ErrSomething if exported",
        "The prefix says up front that it is an error and not just any message, and it is what readers comparing with errors.Is look for.",
        "var notFound = errors.New(\"missing\") becomes var errNotFound = errors.New(\"missing\").",
    ),
    RuleText::new(
        "Variable type that the value already gives (staticcheck QF1011)",
        "drop the type from the declaration: it is inferred from the value",
        "The repeated type adds nothing and has to change in two places if it changes.",
        "var total int64 = sum() becomes var total = sum().",
    ),
    RuleText::new(
        "Redundant type in a variable declaration (staticcheck ST1023)",
        "drop the type from the declaration: it is inferred from the value",
        "The repeated type adds nothing and has to change in two places if it changes.",
        "var names []string = list() becomes var names = list().",
    ),
    RuleText::new(
        "Switch without a tag that always compares the same thing with == (staticcheck QF1002)",
        "move the compared value to the switch and keep only the values in each case",
        "A switch on the value says up front that every case looks at the same thing.",
        "switch { case x == 1: ... } becomes switch x { case 1: ... }.",
    ),
    RuleText::new(
        "Unnecessary assignment to the blank identifier (staticcheck S1005)",
        "drop the extra _",
        "The extra _ only adds noise: the receive or the range already discards that value.",
        "for i, _ := range list becomes for i := range list.",
    ),
    RuleText::new(
        "Empty if or else branch (staticcheck SA9003)",
        "delete the empty branch or fill it in",
        "An empty branch is usually something left half written or a condition that is not needed.",
        "if err != nil { } becomes if err != nil { return err }.",
    ),
    RuleText::new(
        "Infinite for that starts with an if that breaks (staticcheck QF1006)",
        "lift the negated condition into the for and drop the if with the break",
        "The condition in the for says up front how long it repeats.",
        "for { if done { break } ... } becomes for !done { ... }.",
    ),
];

pub fn go(rule: GoRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
