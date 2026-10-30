use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;

// The staticcheck rules the type helper decides on its own.
pub const STATICCHECK_TEXTS: [(GoRule, RuleText); 41] = [
    (
        GoRule::S1009,
        RuleText::new(
            "Nil check before len() that len() already covers (staticcheck S1009)",
            "drop the nil check and keep only the len() comparison",
            "len() of a nil slice, map or channel is zero, so the nil check repeats the length check.",
            "if s == nil || len(s) == 0 becomes if len(s) == 0.",
        ),
    ),
    (
        GoRule::S1011,
        RuleText::new(
            "Loop that appends one by one what a single append can add (staticcheck S1011)",
            "replace the loop with append(dst, src...)",
            "append with ... adds the whole slice at once: shorter, and it grows the destination only once.",
            "for _, v := range src { dst = append(dst, v) } becomes dst = append(dst, src...).",
        ),
    ),
    (
        GoRule::S1017,
        RuleText::new(
            "if that checks a prefix or suffix before trimming it (staticcheck S1017)",
            "call strings.TrimPrefix, TrimSuffix or Replace without the if",
            "The Trim functions already do nothing when the prefix or suffix is missing, so the if checks twice.",
            "if strings.HasPrefix(s, p) { s = s[len(p):] } becomes s = strings.TrimPrefix(s, p).",
        ),
    ),
    (
        GoRule::S1029,
        RuleText::new(
            "Range over []rune(s) without using the index (staticcheck S1029)",
            "range over the string directly",
            "Ranging over a string already yields its runes, without allocating a slice as long as the string.",
            "for _, r := range []rune(s) becomes for _, r := range s.",
        ),
    ),
    (
        GoRule::S1030,
        RuleText::new(
            "Conversion of a bytes.Buffer's content that the buffer already offers (staticcheck S1030)",
            "use buf.String() or buf.Bytes() directly",
            "string(buf.Bytes()) is buf.String(), and []byte(buf.String()) copies twice what buf.Bytes() returns.",
            "string(buf.Bytes()) becomes buf.String().",
        ),
    ),
    (
        GoRule::S1032,
        RuleText::new(
            "sort.Sort(sort.IntSlice(x)) and its pairs (staticcheck S1032)",
            "use sort.Ints, sort.Float64s or sort.Strings",
            "The sort package already has a function for each of these slices; the conversion only adds noise.",
            "sort.Sort(sort.StringSlice(names)) becomes sort.Strings(names).",
        ),
    ),
    (
        GoRule::S1038,
        RuleText::new(
            "fmt.Sprintf passed to a print function that has its own formatted version (staticcheck S1038)",
            "call the f variant (Printf, Errorf, Fatalf...) with the format directly",
            "Formatting into a string and then printing it does in two steps what the f variant does in one.",
            "t.Error(fmt.Sprintf(\"got %d\", n)) becomes t.Errorf(\"got %d\", n).",
        ),
    ),
    (
        GoRule::S1039,
        RuleText::new(
            "fmt.Sprint or fmt.Sprintf of a single string literal (staticcheck S1039)",
            "use the string literal directly",
            "Formatting a constant string without verbs returns the same string.",
            "fmt.Sprintf(\"done\") becomes \"done\".",
        ),
    ),
    (
        GoRule::S1040,
        RuleText::new(
            "Type assertion to the interface the value already has (staticcheck S1040)",
            "drop the assertion",
            "The value already has that interface type: the assertion always succeeds and changes nothing.",
            "var r io.Reader = f; r.(io.Reader).Read(p) becomes r.Read(p).",
        ),
    ),
    (
        GoRule::Sa1006,
        RuleText::new(
            "printf-style call with a dynamic format and no other arguments (staticcheck SA1006)",
            "use the print-style function, or pass the value as an argument to a constant format",
            "If the text has a %, it is read as a verb and the output comes out garbled.",
            "fmt.Printf(message) becomes fmt.Print(message), and fmt.Errorf(message) becomes errors.New(message).",
        ),
    ),
    (
        GoRule::Sa2001,
        RuleText::new(
            "Empty critical section: Lock followed by Unlock (staticcheck SA2001)",
            "put the code that needs the lock between both calls, or remove them",
            "Locking and unlocking right away protects nothing; usually the code in between was lost or the Unlock should be deferred.",
            "mu.Lock(); mu.Unlock() becomes mu.Lock(); defer mu.Unlock() before the guarded code.",
        ),
    ),
    (
        GoRule::Sa4004,
        RuleText::new(
            "Loop that always ends in its first iteration (staticcheck SA4004)",
            "use an if, or move the return or break inside a condition",
            "An unconditional return or break at the end of the body means the loop never repeats, which is rarely what was meant.",
            "for _, x := range xs { if ok(x) { use(x) }; return } runs only for xs[0].",
        ),
    ),
    (
        GoRule::Sa4011,
        RuleText::new(
            "break at the end of a case inside a loop, which only leaves the switch (staticcheck SA4011)",
            "use a labeled break to leave the loop, or remove the break",
            "In Go a case already ends on its own; a plain break there leaves the switch or select, not the surrounding loop.",
            "for { switch x { case 1: break } } keeps looping; use break outer with a label on the for.",
        ),
    ),
    (
        GoRule::Sa4016,
        RuleText::new(
            "Bitwise operation with zero that does nothing (staticcheck SA4016)",
            "check the operand: a flag defined as iota is probably meant to be 1 << iota",
            "x & 0 is always 0 and x | 0 or x ^ 0 is always x; often the constant was meant to be a bit flag.",
            "const FlagA = iota; mode & FlagA is always 0; declare FlagA = 1 << iota.",
        ),
    ),
    (
        GoRule::Sa6002,
        RuleText::new(
            "sync.Pool.Put with a value that is not a pointer (staticcheck SA6002)",
            "store a pointer in the pool, such as *[]byte",
            "Putting a non-pointer value into the pool's interface allocates on every call, which is what the pool was meant to avoid.",
            "pool.Put(buf) with buf []byte becomes pool.Put(&buf).",
        ),
    ),
    (
        GoRule::Sa6003,
        RuleText::new(
            "Conversion of a string to []rune just to range over it (staticcheck SA6003)",
            "range over the string directly",
            "The conversion allocates a slice as long as the string; ranging over the string yields the same runes without it.",
            "for _, r := range []rune(s) becomes for _, r := range s.",
        ),
    ),
    (
        GoRule::Sa6005,
        RuleText::new(
            "Case-insensitive comparison with strings.ToLower or ToUpper (staticcheck SA6005)",
            "use strings.EqualFold",
            "Lowering both strings allocates two copies, and EqualFold compares Unicode case correctly without them.",
            "strings.ToLower(a) == strings.ToLower(b) becomes strings.EqualFold(a, b).",
        ),
    ),
    (
        GoRule::Sa9009,
        RuleText::new(
            "Compiler directive with a space after // (staticcheck SA9009)",
            "remove the space: //go:directive",
            "The compiler only reads directives written as //go: with no space, so this one is silently ignored.",
            "// go:noinline becomes //go:noinline.",
        ),
    ),
    (
        GoRule::St1008,
        RuleText::new(
            "error that is not the last result (staticcheck ST1008)",
            "return the error last",
            "By convention the error goes last, and callers and tools expect it there.",
            "func load() (error, []byte) becomes func load() ([]byte, error).",
        ),
    ),
    (
        GoRule::Qf1007,
        RuleText::new(
            "Boolean declared and then set in an if that can be merged (staticcheck QF1007)",
            "assign the condition in the declaration",
            "x := false followed by if cond { x = true } is the same as x := cond, in one line.",
            "found := false; if n > 0 { found = true } becomes found := n > 0.",
        ),
    ),
    (
        GoRule::S1004,
        RuleText::new(
            "bytes.Compare compared with zero (staticcheck S1004)",
            "use bytes.Equal",
            "bytes.Equal says the same thing directly and is faster.",
            "bytes.Compare(a, b) == 0 becomes bytes.Equal(a, b).",
        ),
    ),
    (
        GoRule::S1006,
        RuleText::new(
            "for true {} instead of for {} (staticcheck S1006)",
            "drop the true",
            "In Go an infinite loop is written for {}; the condition adds nothing.",
            "for true { ... } becomes for { ... }.",
        ),
    ),
    (
        GoRule::S1010,
        RuleText::new(
            "Slice up to len() of itself (staticcheck S1010)",
            "drop the second index",
            "s[a:len(s)] is identical to s[a:].",
            "s[1:len(s)] becomes s[1:].",
        ),
    ),
    (
        GoRule::S1012,
        RuleText::new(
            "time.Now().Sub(t) instead of time.Since(t) (staticcheck S1012)",
            "use time.Since",
            "time.Since is the standard way to say it and reads better.",
            "time.Now().Sub(start) becomes time.Since(start).",
        ),
    ),
    (
        GoRule::S1023,
        RuleText::new(
            "Redundant break at the end of a case or return at the end of a function (staticcheck S1023)",
            "remove it",
            "A Go case does not fall through and a function without results returns at its end; the statement repeats what already happens.",
            "case 1: f(); break becomes case 1: f().",
        ),
    ),
    (
        GoRule::S1024,
        RuleText::new(
            "t.Sub(time.Now()) instead of time.Until(t) (staticcheck S1024)",
            "use time.Until",
            "time.Until is the standard way to say it and reads better.",
            "deadline.Sub(time.Now()) becomes time.Until(deadline).",
        ),
    ),
    (
        GoRule::S1028,
        RuleText::new(
            "errors.New(fmt.Sprintf(...)) instead of fmt.Errorf (staticcheck S1028)",
            "use fmt.Errorf with the same arguments",
            "fmt.Errorf formats and builds the error in one step, and can also wrap with %w.",
            "errors.New(fmt.Sprintf(\"bad id %d\", id)) becomes fmt.Errorf(\"bad id %d\", id).",
        ),
    ),
    (
        GoRule::S1033,
        RuleText::new(
            "if that checks a key before delete (staticcheck S1033)",
            "call delete without the if",
            "delete on a missing key does nothing, so the guard is extra.",
            "if _, ok := m[k]; ok { delete(m, k) } becomes delete(m, k).",
        ),
    ),
    (
        GoRule::S1037,
        RuleText::new(
            "select with a single case on time.After instead of time.Sleep (staticcheck S1037)",
            "use time.Sleep",
            "A select that only waits for time.After is an elaborate way to sleep.",
            "select { case <-time.After(d): } becomes time.Sleep(d).",
        ),
    ),
    (
        GoRule::Sa1012,
        RuleText::new(
            "nil passed as a context.Context (staticcheck SA1012)",
            "pass context.TODO() or the caller's context",
            "Code that receives a context expects it to work: a nil one panics as soon as it is used.",
            "client.Do(nil, req) becomes client.Do(ctx, req).",
        ),
    ),
    (
        GoRule::Sa1013,
        RuleText::new(
            "Seek with the io.Seek* constant as the offset (staticcheck SA1013)",
            "swap the arguments: Seek(offset, whence)",
            "The first argument of Seek is the offset; passing io.SeekStart there moves to a wrong position.",
            "f.Seek(io.SeekStart, 0) becomes f.Seek(0, io.SeekStart).",
        ),
    ),
    (
        GoRule::Sa2000,
        RuleText::new(
            "sync.WaitGroup.Add inside the goroutine it counts (staticcheck SA2000)",
            "call Add before starting the goroutine",
            "The goroutine may start after Wait has already returned, so Wait does not wait for it.",
            "go func() { wg.Add(1); ... }() becomes wg.Add(1); go func() { ... }().",
        ),
    ),
    (
        GoRule::Sa3001,
        RuleText::new(
            "Assignment to b.N in a benchmark (staticcheck SA3001)",
            "do not assign b.N; let the testing package choose it",
            "The testing package sets b.N to get a reliable measurement; overwriting it makes the result meaningless.",
            "b.N = 1000 is removed and the loop uses for range b.N or b.Loop().",
        ),
    ),
    (
        GoRule::Sa4001,
        RuleText::new(
            "&*x or *&x, which do not copy x (staticcheck SA4001)",
            "use x directly, or copy it explicitly into a new variable",
            "The compiler simplifies both forms to x, so whoever wrote them expecting a copy gets the same pointer.",
            "p2 := &*p becomes copy := *p; p2 := &copy.",
        ),
    ),
    (
        GoRule::Sa4013,
        RuleText::new(
            "Double negation of a boolean (staticcheck SA4013)",
            "remove both negations, or keep one if that was the intent",
            "!!x is x; it is usually a typo.",
            "if !!ok becomes if ok.",
        ),
    ),
    (
        GoRule::Sa4021,
        RuleText::new(
            "append with nothing to append (staticcheck SA4021)",
            "add the missing elements or use the slice directly",
            "x = append(y) is the same as x = y; usually an argument was forgotten.",
            "items = append(items) becomes items = append(items, item).",
        ),
    ),
    (
        GoRule::Sa4022,
        RuleText::new(
            "Comparison of a variable's address with nil (staticcheck SA4022)",
            "compare the variable itself, or remove the check",
            "The address of a variable is never nil, so the comparison always gives the same result.",
            "if &cfg == nil becomes if cfg == nil, if cfg is a pointer.",
        ),
    ),
    (
        GoRule::Sa4024,
        RuleText::new(
            "len or cap compared as less than zero (staticcheck SA4024)",
            "fix the comparison: len and cap are never negative",
            "The condition never holds; usually it was meant to be == 0.",
            "if len(items) < 0 becomes if len(items) == 0.",
        ),
    ),
    (
        GoRule::Sa4028,
        RuleText::new(
            "x % 1, which is always zero (staticcheck SA4028)",
            "check the divisor",
            "Any integer modulo 1 is 0, so the expression is a constant in disguise.",
            "if n % 1 == 0 becomes if n % 2 == 0, if parity was meant.",
        ),
    ),
    (
        GoRule::Sa5002,
        RuleText::new(
            "Empty for loop that spins (staticcheck SA5002)",
            "wait on a channel, a sync primitive or time.Sleep instead",
            "An empty loop uses a whole CPU, and if its condition depends on another goroutine it is also a data race.",
            "for !done {} becomes <-doneChannel.",
        ),
    ),
    (
        GoRule::Sa6006,
        RuleText::new(
            "io.WriteString with a []byte converted to string (staticcheck SA6006)",
            "call w.Write with the bytes",
            "Converting the bytes to a string copies them just so WriteString can write them.",
            "io.WriteString(w, string(data)) becomes w.Write(data).",
        ),
    ),
];
