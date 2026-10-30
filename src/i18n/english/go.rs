mod staticcheck;

use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;
use staticcheck::STATICCHECK_TEXTS;

const UNKNOWN: RuleText = RuleText::new(
    "golangci-lint rule",
    "follow the rule's advice",
    "golangci-lint reports it by default.",
    "See the golangci-lint documentation.",
);
const TEXTS: [RuleText; 30] = [
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
    RuleText::new(
        "Package imported more than once (staticcheck ST1019)",
        "keep a single import of the package",
        "Two names for the same package are confusing: readers cannot tell whether they are the same.",
        "import ( \"fmt\"; f \"fmt\" ) becomes import \"fmt\".",
    ),
    RuleText::new(
        "context.WithValue key of a built-in type (staticcheck SA1029)",
        "define your own type for the key",
        "With a string or an int as key, another package can use the same one and overwrite the value.",
        "context.WithValue(ctx, \"user\", u) becomes context.WithValue(ctx, userKey{}, u).",
    ),
    RuleText::new(
        "Constant group where only the first one has a type (staticcheck SA9004)",
        "give every constant in the group the type",
        "The others are untyped and not of the first one's type, even if it looks like they are.",
        "const ( A T = 1; B = 2 ) becomes const ( A T = 1; B T = 2 ).",
    ),
    RuleText::new(
        "time.Time compared with == (staticcheck QF1009)",
        "use Equal",
        "With == the location and the monotonic reading count too: the same instant can compare different.",
        "a == b becomes a.Equal(b).",
    ),
    RuleText::new(
        "HTTP status code written as a number (staticcheck ST1013)",
        "use the net/http constant",
        "The name says what the code means without having to know it by heart.",
        "http.Error(w, msg, 403) becomes http.Error(w, msg, http.StatusForbidden).",
    ),
    RuleText::new(
        "select with a single case (staticcheck S1000)",
        "use the plain receive or send, or a for range inside a for",
        "A select with a single case does the same as the operation alone, in a roundabout way.",
        "for { select { case v := <-ch: use(v) } } becomes for v := range ch { use(v) }.",
    ),
    RuleText::new(
        "Yoda condition: the constant on the left (staticcheck ST1017)",
        "put what varies on the left and the constant on the right",
        "Go does not need it (assigning in a condition does not compile) and it reads backwards.",
        "if 5 == x becomes if x == 5.",
    ),
    RuleText::new(
        "Receiver named self, this or _ (staticcheck ST1006)",
        "name it for what it is (one or two letters of the type), or drop the name if unused",
        "In Go the receiver is just another parameter and its short name says which type it is.",
        "func (this *Server) Start() becomes func (s *Server) Start().",
    ),
    RuleText::new(
        "if returning true or false followed by the opposite (staticcheck S1008)",
        "return the condition directly",
        "The if adds nothing: the condition already is the bool being returned.",
        "if x > 0 { return true }; return false becomes return x > 0.",
    ),
    RuleText::new(
        "Comparison with true or false (staticcheck S1002)",
        "use the bool directly, negated if needed",
        "Comparing a bool with a constant says the same thing twice.",
        "if ok == true becomes if ok.",
    ),
    RuleText::new(
        "Code that nothing uses (unused)",
        "delete it, or use it if it was left unwired",
        "An unused function, type, field or constant still has to be read and maintained, and sometimes it is something someone forgot to call.",
        "func oldFormat() string { ... } with no call anywhere: delete it.",
    ),
    RuleText::new(
        "Type switch that repeats the assertion in its cases (staticcheck S1034)",
        "assign the result in the switch and use it in the cases",
        "With switch v := x.(type) each case already has the typed value; repeating x.(T) is noise.",
        "switch x.(type) { case int: f(x.(int)) } becomes switch v := x.(type) { case int: f(v) }.",
    ),
    RuleText::new(
        "Redundant net/http.CanonicalHeaderKey call on an HTTP header method (staticcheck S1035)",
        "pass the header name directly to Add, Del, Get or Set",
        "HTTP header methods already canonicalize header names, so calling CanonicalHeaderKey first adds no behavior.",
        "h.Set(http.CanonicalHeaderKey(name), value) becomes h.Set(name, value).",
    ),
];

pub fn go(rule: GoRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or_else(|| {
        STATICCHECK_TEXTS
            .iter()
            .find(|(candidate, _)| *candidate == rule)
            .map_or(UNKNOWN, |(_, text)| *text)
    })
}
