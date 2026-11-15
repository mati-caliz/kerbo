use crate::i18n::catalog::RuleText;
use crate::rules::JavaRule;

pub const RULE_COUNT: usize = 16;

const RULE_TEXTS: [(JavaRule, RuleText); RULE_COUNT] = [
    (
        JavaRule::UnnecessaryUnicodeEscape,
        RuleText::new(
            "Unicode escape for a character that can be written as is",
            "write the character instead of the escape",
            "A \\u0041 makes the reader look up which letter it is; the file is UTF-8 and the character reads directly. Like IntelliJ's UnnecessaryUnicodeEscape: control characters, unusual spaces, invisible, unassigned and combining characters stay escaped, since they do not show well when written.",
            "String name = \"Jos\\u00e9\"; becomes String name = \"José\";.",
        ),
    ),
    (
        JavaRule::Java8ListSort,
        RuleText::new(
            "Collections.sort(list, comparator) where list.sort(comparator) is enough",
            "call sort on the list",
            "Since Java 8 a list sorts itself with List.sort, which some implementations do faster than copying to an array. Like IntelliJ's Java8ListSort: only with release 8 or later.",
            "Collections.sort(orders, byDate); becomes orders.sort(byDate);.",
        ),
    ),
    (
        JavaRule::SizeReplaceableByIsEmpty,
        RuleText::new(
            "size() or length() compared with 0 or 1 where isEmpty() is enough",
            "use isEmpty(), negated when the comparison asks whether there is anything",
            "isEmpty() says what is being asked without counting, and in some collections it is cheaper than counting. Like IntelliJ's SizeReplaceableByIsEmpty: only when the declared type has isEmpty() (String, the Java and guava collections and maps, CharSequence since Java 15).",
            "if (orders.size() == 0) becomes if (orders.isEmpty()), and name.length() > 0, !name.isEmpty().",
        ),
    ),
    (
        JavaRule::ManualMinMaxCalculation,
        RuleText::new(
            "Minimum or maximum computed by hand with a ternary or an if",
            "use Math.min or Math.max",
            "Math.min(a, b) says what is computed; the ternary or the if makes the reader go through the comparison and both branches, and the sign is easy to flip. Like IntelliJ's ManualMinMaxCalculation: only when both sides have the same numeric type (int, long, float or double).",
            "int cap = (orders < limit) ? orders : limit; becomes int cap = Math.min(orders, limit);.",
        ),
    ),
    (
        JavaRule::DefaultAnnotationParam,
        RuleText::new(
            "Annotation element written with the value it already has by default",
            "remove the element",
            "Repeating the default value is noise and makes the reader check whether it differs. Like IntelliJ's DefaultAnnotationParam: only for annotations declared in the project, where the default can be seen.",
            "@Retry(attempts = 3) when attempts() default 3 becomes @Retry.",
        ),
    ),
    (
        JavaRule::Java8MapApi,
        RuleText::new(
            "Value read with get, computed if null and stored with put",
            "use Map.computeIfAbsent",
            "computeIfAbsent says in one call what the get, the null check and the put spread over several lines, and looks the key up once. Like IntelliJ's Java8MapApi in the computeIfAbsent form: only when the declared type is a Map, the release is 8 or later and the computed value has no side effects (an array or a literal).",
            "Class<?>[] result = cache.get(type); if (result == null) { result = new Class[] { type }; cache.put(type, result); } becomes Class<?>[] result = cache.computeIfAbsent(type, key -> new Class[] { key });.",
        ),
    ),
    (
        JavaRule::FinalStaticMethod,
        RuleText::new(
            "Static method declared final",
            "remove the final",
            "A static method is never overridden: final only stops a subclass from declaring one with the same signature, which is rarely intended, and usually reveals the mistaken idea that it prevents an override. Like IntelliJ's FinalStaticMethod, which Checkstyle and PMD lack.",
            "public static final int twice(int value) becomes public static int twice(int value).",
        ),
    ),
    (
        JavaRule::TransientStaticField,
        RuleText::new(
            "Static field declared transient",
            "remove the transient",
            "Serialization never writes static fields, so transient changes nothing and suggests it does. Like IntelliJ's UnnecessaryModifier, which Checkstyle and PMD lack.",
            "private static transient Logger log; becomes private static Logger log;.",
        ),
    ),
    (
        JavaRule::TextBlockMigration,
        RuleText::new(
            "String literals concatenated with two or more line breaks, where a text block reads better",
            "write it as a text block",
            "A text block shows the text as it will come out, without the line break escapes and the + at the end of each line. Like IntelliJ's TextBlockMigration: only when every part is a literal, there are at least two escaped line breaks and no comment in between, with release 15 or later.",
            "Two literals in a row that end in a line break, like a select query and its from, become a text block with the two lines.",
        ),
    ),
    (
        JavaRule::SequencedCollectionMethod,
        RuleText::new(
            "get(0), get(size() - 1), remove(0), add(0, x) or iterator().next() where Java 21 has getFirst, getLast, removeFirst or addFirst",
            "call the sequenced collection method",
            "getFirst() says which element is wanted; get(0) and the size() - 1 arithmetic make the reader work it out. Like IntelliJ's SequencedCollectionMethodCanBeUsed: with release 21 or later, when the declared type is a list or another sequenced collection, and not in a series like get(0), get(1).",
            "orders.get(orders.size() - 1) becomes orders.getLast().",
        ),
    ),
    (
        JavaRule::MathClampMigration,
        RuleText::new(
            "Math.min and Math.max nested where Java 21 has Math.clamp",
            "use Math.clamp",
            "Math.clamp(value, low, high) says the value is kept within a range; nested min and max make the reader work out which bound is which. Like IntelliJ's MathClampMigration: with release 21 or later, when something inside the inner call is not a constant.",
            "Math.max(0, Math.min(60, minutes)) becomes Math.clamp(minutes, 0, 60).",
        ),
    ),
    (
        JavaRule::StringRepeatCanBeUsed,
        RuleText::new(
            "append(text.repeat(n)) where Java 21 has StringBuilder.repeat",
            "call repeat on the builder",
            "builder.repeat(text, n) writes the copies straight into the builder instead of building an intermediate String first. Like IntelliJ's StringRepeatCanBeUsed in its Java 21 form: the receiver is a declared StringBuilder or StringBuffer and the repeated value is a String.",
            "line.append(\"-\".repeat(width)) becomes line.repeat(\"-\", width).",
        ),
    ),
    (
        JavaRule::PatternVariableCanBeUsed,
        RuleText::new(
            "Local variable initialized with a cast that an earlier instanceof already guarantees",
            "declare it in the instanceof as a pattern variable",
            "With Java 16 patterns, value instanceof Order order checks and declares in one step; the separate cast repeats the type and can fall out of step with the check. Like IntelliJ's PatternVariableCanBeUsed: the instanceof is the condition of the enclosing if or loop, or an earlier if in the same block that leaves when it fails, and the cast value is not reassigned in between.",
            "if (value instanceof Order) { Order order = (Order) value; ... } becomes if (value instanceof Order order) { ... }.",
        ),
    ),
    (
        JavaRule::ReadWriteString,
        RuleText::new(
            "Bytes read or written with Files only to turn them into a String, where Files.readString and Files.writeString do it",
            "use Files.readString or Files.writeString",
            "Files.readString(path, charset) says in one call what new String(Files.readAllBytes(path), charset) builds in two. Like IntelliJ's ReadWriteStringCanBeUsed: readString since Java 11 and writeString since Java 12, when the charset is a Charset.",
            "new String(Files.readAllBytes(path), UTF_8) becomes Files.readString(path, UTF_8).",
        ),
    ),
    (
        JavaRule::Java9CollectionFactory,
        RuleText::new(
            "Unmodifiable copy or fixed list built by hand where List.of or List.copyOf do it",
            "use List.of, Set.of, Map.of or their copyOf",
            "List.copyOf(other) says in one call what Collections.unmodifiableList(new ArrayList<>(other)) builds in two, and the result rejects nulls. Like IntelliJ's Java9CollectionFactory: copyOf since Java 10 when the copied value is declared as a collection; of since Java 9 when no element can be null.",
            "Collections.unmodifiableList(new ArrayList<>(orders)) becomes List.copyOf(orders).",
        ),
    ),
    (
        JavaRule::EnhancedSwitchMigration,
        RuleText::new(
            "Old switch that returns in every branch, where a switch expression says it at once",
            "return a switch expression with case ... ->",
            "return switch (kind) { case A -> 1; default -> 0; }; shows that every branch gives a value and the compiler checks none is missing. Like IntelliJ's EnhancedSwitchMigration in its default form: every branch, of up to two statements, ends in return or throw, and without default a return or throw follows the switch.",
            "switch (kind) { case A: return 1; default: return 0; } becomes return switch (kind) { case A -> 1; default -> 0; };.",
        ),
    ),
];

pub fn rule_text(rule: JavaRule) -> Option<RuleText> {
    RULE_TEXTS
        .iter()
        .find(|(known, _)| *known == rule)
        .map(|(_, text)| *text)
}
