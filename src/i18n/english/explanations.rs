use crate::i18n::Explanation;
use crate::rules::RuleId;

const FILE_LENGTH: Explanation = Explanation {
    fix: "split the file by responsibility",
    why: "A long file mixes responsibilities: things are hard to find and every change touches more than it should.",
    example: "Move each group of functions that change together to its own module, and keep the original as a facade if needed.",
};
const FUNCTION_LENGTH: Explanation = Explanation {
    fix: "extract steps into functions with their own names",
    why: "A long function does several things: its name stops describing it and tests cannot target each part.",
    example: "Every block with a comment explaining what it does is a function waiting for its name.",
};
const CYCLOMATIC_COMPLEXITY: Explanation = Explanation {
    fix: "replace branches with early returns, lookup tables or polymorphism",
    why: "Every if, case, && or || is one more path: with many of them, no test covers them all.",
    example: "A switch that picks an action by type becomes a map from type to function.",
};
const COGNITIVE_COMPLEXITY: Explanation = Explanation {
    fix: "flatten the nesting with early returns and helper functions",
    why: "Nesting costs more than the number of branches: every level is context the reader has to keep in mind.",
    example: "if (valid) { ...everything... } becomes if (!valid) return; ...everything...",
};
const PARAMETER_COUNT: Explanation = Explanation {
    fix: "group the parameters that travel together into an object",
    why: "Many positional parameters get mixed up in order and signal that the function does too much.",
    example: "create(name, email, age, country, plan) becomes create(data: NewUser).",
};
const SUPPRESSION_ADDED: Explanation = Explanation {
    fix: "fix the cause instead of silencing the tool",
    why: "A new eslint-disable, noqa, nolint or @SuppressWarnings hides the problem instead of solving it.",
    example: "If it is a confirmed false positive, it goes as an exception in kerbo.toml with its reason, in plain sight.",
};
const TEST_DISABLED: Explanation = Explanation {
    fix: "fix the test; do not skip it or focus it",
    why: "A skip stops verifying the behavior; an only leaves every other test in the file unrun.",
    example: "If the test is wrong, correct it; if the behavior changed, update what it verifies.",
};
const TEST_REMOVED: Explanation = Explanation {
    fix: "restore the tests or justify the removal",
    why: "Deleting a failing test makes the gate pass without fixing anything.",
    example: "If the code it tested is gone, say so in the commit; otherwise, the test comes back.",
};
const NESTING_DEPTH: Explanation = Explanation {
    fix: "move the inner block to a function or invert the condition with an early return",
    why: "Every nesting level is one more condition the reader has to keep in mind to understand a line.",
    example: "for (...) { if (ok) { ... } } becomes for (...) { if (!ok) continue; ... }",
};
const MAGIC_NUMBER: Explanation = Explanation {
    fix: "give it a name: a constant that says what the number is",
    why: "A bare 86400 forces the reader to guess what it stands for and, when it changes, to hunt down every copy.",
    example: "if (age >= 18) becomes const LEGAL_AGE = 18; if (age >= LEGAL_AGE).",
};
const SHORT_NAME: Explanation = Explanation {
    fix: "use a name that says what the variable holds",
    why: "With a single letter you have to read the whole block to know what it is, and l, O and I look like 1 and 0.",
    example: "for (const o of orders) becomes for (const order of orders).",
};
const DUPLICATE_CODE: Explanation = Explanation {
    fix: "extract the repeated code into a shared function or type and call it from both places",
    why: "A copied block gets fixed in one place and stays broken in the other: every copy is a change you have to remember to repeat.",
    example: "Two services that validate an order the same way switch to the same validateOrder(order).",
};
const COMMENTED_CODE: Explanation = Explanation {
    fix: "delete the commented-out code; if you need it back, it is in the git history",
    why: "Commented-out code is neither compiled nor tested: it rots silently and the reader cannot tell whether it still holds.",
    example: "# total = sum(orders) gets deleted; if it was an alternative, it goes in the commit message.",
};
const TYPE_ESCAPE: Explanation = Explanation {
    fix: "type the value; with external data, unknown plus validation",
    why: "any and as unknown as turn the compiler off right where it is needed most.",
    example: "const data: any = await res.json() becomes const data = schema.parse(await res.json()).",
};

const UNUSED_IMPORT: Explanation = Explanation {
    fix: "delete the import",
    why: "An unused import suggests the file depends on something it does not use, and it adds noise to every diff of the header.",
    example: "import java.util.List; with no List anywhere in the file is deleted.",
};
const REDUNDANT_IMPORT: Explanation = Explanation {
    fix: "delete the duplicated import, the one from java.lang or the one from the same package",
    why: "Classes in java.lang and in the same package are visible without an import, and repeating an import adds nothing.",
    example: "import java.lang.String; is deleted: String is already in sight.",
};
const STAR_IMPORT: Explanation = Explanation {
    fix: "import each class by name",
    why: "A wildcard import hides where each name comes from, and a new class in the package can clash with one you already use.",
    example: "import java.util.*; becomes import java.util.List; import java.util.Map;",
};
const REDUNDANT_MODIFIER: Explanation = Explanation {
    fix: "delete the modifier",
    why: "Repeating what the language already assumes (public in an interface, final on a private method) is noise and makes the reader wonder if it means more.",
    example: "In an interface, public abstract void save(); becomes void save();",
};
const FINAL_PARAMETER: Explanation = Explanation {
    fix: "mark the parameter as final",
    why: "Reassigning a parameter overwrites the value the function was called with and confuses whoever reads the rest of the body.",
    example: "void charge(Order order) becomes void charge(final Order order).",
};
const FINAL_CLASS: Explanation = Explanation {
    fix: "declare the class final",
    why: "If its constructors are private nobody outside can extend it: final says so in the signature and lets the compiler enforce it.",
    example: "class Dates { private Dates() {} } becomes final class Dates { private Dates() {} }.",
};

pub fn explanation(rule: RuleId) -> Explanation {
    match rule {
        RuleId::FileLength => FILE_LENGTH,
        RuleId::FunctionLength => FUNCTION_LENGTH,
        RuleId::CyclomaticComplexity => CYCLOMATIC_COMPLEXITY,
        RuleId::CognitiveComplexity => COGNITIVE_COMPLEXITY,
        RuleId::ParameterCount => PARAMETER_COUNT,
        RuleId::SuppressionAdded => SUPPRESSION_ADDED,
        RuleId::TestDisabled => TEST_DISABLED,
        RuleId::TestRemoved => TEST_REMOVED,
        RuleId::NestingDepth => NESTING_DEPTH,
        RuleId::MagicNumber => MAGIC_NUMBER,
        RuleId::ShortName => SHORT_NAME,
        RuleId::DuplicateCode => DUPLICATE_CODE,
        RuleId::CommentedCode => COMMENTED_CODE,
        RuleId::CommittedSecret => super::security::COMMITTED_SECRET,
        RuleId::VulnerableDependency => super::security::VULNERABLE_DEPENDENCY,
        RuleId::TypeEscape => TYPE_ESCAPE,
        RuleId::UnusedImport => UNUSED_IMPORT,
        RuleId::RedundantImport => REDUNDANT_IMPORT,
        RuleId::StarImport => STAR_IMPORT,
        RuleId::RedundantModifier => REDUNDANT_MODIFIER,
        RuleId::FinalParameter => FINAL_PARAMETER,
        RuleId::FinalClass => FINAL_CLASS,
        RuleId::Migration(rule) => super::deploy::migration(rule).explanation,
        RuleId::Dockerfile(rule) => super::dockerfile::dockerfile(rule).explanation,
        RuleId::Compose(rule) => super::deploy::compose(rule).explanation,
        RuleId::Python(rule) => super::python::python(rule).explanation,
    }
}
