use crate::i18n::catalog::RuleText;
use crate::rules::JavaRule;

const UNKNOWN: RuleText = RuleText::new(
    "Checkstyle rule",
    "follow the rule's guidance",
    "Checkstyle reports it with sun_checks.xml.",
    "See the Checkstyle documentation.",
);
const TEXTS: [RuleText; JavaRule::ALL.len()] = [
    RuleText::new(
        "Type whose name is not PascalCase",
        "name it with an uppercase initial, letters and digits",
        "Classes, interfaces, enums, records and annotations go in PascalCase (^[A-Z][a-zA-Z0-9]*$), so they stand apart from variables and methods at a glance.",
        "class order_item becomes class OrderItem.",
    ),
    RuleText::new(
        "Method whose name is not camelCase, or that is named like its class",
        "name it with a lowercase initial, letters and digits",
        "Methods go in camelCase (^[a-z][a-zA-Z0-9]*$); one named like the class is mistaken for a constructor. Methods with @Override are skipped: whoever declares them decides the name.",
        "void Do_thing() becomes void doThing().",
    ),
    RuleText::new(
        "Constant (static final) that is not in UPPER_SNAKE_CASE",
        "name it in uppercase, with underscores between words",
        "A static final field, and any field of an interface, is a constant and goes in ^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$; serialVersionUID is left out.",
        "static final int maxSize = 10; becomes static final int MAX_SIZE = 10;.",
    ),
    RuleText::new(
        "Static variable whose name is not camelCase",
        "name it with a lowercase initial, letters and digits",
        "A static field that is not final goes in camelCase (^[a-z][a-zA-Z0-9]*$), like any variable: uppercase is for constants.",
        "static int Counter; becomes static int counter;.",
    ),
    RuleText::new(
        "Field whose name is not camelCase",
        "name it with a lowercase initial, letters and digits",
        "Instance fields go in camelCase (^[a-z][a-zA-Z0-9]*$), without underscores or prefixes like m_.",
        "private int m_count; becomes private int count;.",
    ),
    RuleText::new(
        "Parameter whose name is not camelCase",
        "name it with a lowercase initial, letters and digits",
        "Method and constructor parameters go in camelCase (^[a-z][a-zA-Z0-9]*$).",
        "void put(String Key) becomes void put(String key).",
    ),
    RuleText::new(
        "Long literal with a lowercase l",
        "use an uppercase L",
        "A lowercase l looks like a 1: 10l reads as 101.",
        "long total = 10l; becomes long total = 10L;.",
    ),
    RuleText::new(
        "Empty statement: a stray ;",
        "remove the ; or fill in the body",
        "A stray ; does nothing, and after a for, a while or an if it leaves the next block outside the loop or the condition.",
        "for (int i = 0; i < n; i++); { add(i); } becomes for (int i = 0; i < n; i++) { add(i); }.",
    ),
];

pub fn java(rule: JavaRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
