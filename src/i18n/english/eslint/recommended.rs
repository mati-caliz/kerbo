use crate::i18n::catalog::RuleText;
use crate::rules::EslintRule;

pub const RECOMMENDED_TEXTS: [(EslintRule, RuleText); 19] = [
    (
        EslintRule::ForDirection,
        RuleText::new(
            "for loop whose counter moves away from its stop condition",
            "change the update, or the condition",
            "The loop either never runs or never stops.",
            "for (let i = 0; i < 10; i--) becomes for (let i = 0; i < 10; i++).",
        ),
    ),
    (
        EslintRule::NoDeleteVar,
        RuleText::new(
            "delete on a variable",
            "assign undefined, or remove the delete",
            "delete only removes object properties; on a variable it does nothing or is an error.",
            "delete a; becomes a = undefined;.",
        ),
    ),
    (
        EslintRule::NoDupeArgs,
        RuleText::new(
            "Repeated parameter name in a function",
            "rename one of the parameters",
            "The last one hides the others, and in strict mode it does not even parse.",
            "function f(a, a) becomes function f(a, b).",
        ),
    ),
    (
        EslintRule::NoDupeClassMembers,
        RuleText::new(
            "Repeated class member",
            "rename it, or remove one",
            "The last definition silently replaces the previous ones.",
            "a() {} a() {} keeps only one.",
        ),
    ),
    (
        EslintRule::NoEmptyStaticBlock,
        RuleText::new(
            "Empty static block",
            "remove it, or add a comment that explains it",
            "It does nothing, so it is probably unfinished code.",
            "static {} gets removed.",
        ),
    ),
    (
        EslintRule::NoExAssign,
        RuleText::new(
            "Assignment to the exception of a catch",
            "use another variable",
            "The original error is lost, and later handlers see something else.",
            "catch (e) { e = 1; } uses another variable.",
        ),
    ),
    (
        EslintRule::NoNewNativeNonconstructor,
        RuleText::new(
            "new on Symbol or BigInt",
            "call it without new",
            "They are not constructors: new throws a TypeError.",
            "new Symbol() becomes Symbol().",
        ),
    ),
    (
        EslintRule::NoNonoctalDecimalEscape,
        RuleText::new(
            "\\8 or \\9 in a string",
            "write the digit without the backslash",
            "They are not valid escapes, and strict mode forbids them in templates.",
            "\"\\8\" becomes \"8\".",
        ),
    ),
    (
        EslintRule::NoObjCalls,
        RuleText::new(
            "Call to a global that is not a function",
            "call one of its members",
            "Math, JSON, Reflect and the like are plain objects: calling them throws.",
            "Math() becomes Math.floor(x).",
        ),
    ),
    (
        EslintRule::NoOctal,
        RuleText::new(
            "Number with a leading 0",
            "remove the 0, or write 0o",
            "It is read as octal: 010 is 8.",
            "010 becomes 0o10.",
        ),
    ),
    (
        EslintRule::NoUnsafeNegation,
        RuleText::new(
            "! on the left of in or instanceof",
            "use parentheses: !(a in b)",
            "! applies first, so it compares a boolean.",
            "!a in b becomes !(a in b).",
        ),
    ),
    (
        EslintRule::NoWith,
        RuleText::new(
            "with statement",
            "use the object explicitly, or a variable",
            "It makes it impossible to know where each name comes from, and strict mode forbids it.",
            "with (a) b; becomes a.b;.",
        ),
    ),
    (
        EslintRule::NoDuplicateEnumValues,
        RuleText::new(
            "Repeated value in an enum",
            "give each member its own value",
            "Two names with the same value cannot be told apart.",
            "A = 1, B = 1 becomes A = 1, B = 2.",
        ),
    ),
    (
        EslintRule::NoExtraNonNullAssertion,
        RuleText::new(
            "! that adds nothing",
            "remove the extra !",
            "a!! is the same as a!, and a!?.b contradicts itself.",
            "a!!; becomes a!;.",
        ),
    ),
    (
        EslintRule::NoMisusedNew,
        RuleText::new(
            "Constructor declared the wrong way",
            "use a class, or a type with new (): T",
            "An interface's constructor() is a method, and a class's new() too: neither builds anything.",
            "interface A { new(): A } becomes class A {}.",
        ),
    ),
    (
        EslintRule::NoUnnecessaryTypeConstraint,
        RuleText::new(
            "extends any or extends unknown in a type parameter",
            "remove the constraint",
            "Type parameters already accept anything.",
            "<T extends any> becomes <T>.",
        ),
    ),
    (
        EslintRule::NoUnsafeDeclarationMerging,
        RuleText::new(
            "Class and interface with the same name",
            "rename one of them",
            "The compiler assumes the interface's members exist, without checking they are initialized.",
            "interface A {} class A {} uses two names.",
        ),
    ),
    (
        EslintRule::PreferAsConst,
        RuleText::new(
            "Literal type that repeats the value",
            "use as const",
            "It repeats the same literal twice, and they can get out of sync.",
            "'a' as 'a' becomes 'a' as const.",
        ),
    ),
    (
        EslintRule::PreferNamespaceKeyword,
        RuleText::new(
            "module where namespace is meant",
            "use namespace",
            "module is the old name, and looks like an external module declaration.",
            "module A {} becomes namespace A {}.",
        ),
    ),
];
