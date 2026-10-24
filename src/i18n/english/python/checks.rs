use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: chequeos de excepciones, loops y llamadas.
pub const CHECK_TEXTS: [(PythonRule, RuleText); 4] = [
    (
        PythonRule::TRY203,
        RuleText::new(
            "try whose except clauses only re-raise",
            "remove the try, or handle the error in the except",
            "An except that only raises the same exception again does nothing but add indentation.",
            "try:\n    f()\nexcept ValueError:\n    raise becomes f().",
        ),
    ),
    (
        PythonRule::PLE0704,
        RuleText::new(
            "Bare raise outside an except clause",
            "raise a specific exception",
            "A bare raise outside an except has no active exception to re-raise and fails with RuntimeError.",
            "raise becomes raise ValueError(\"...\").",
        ),
    ),
    (
        PythonRule::SIM210,
        RuleText::new(
            "True if condition else False",
            "use bool(condition), or the condition itself if it is a comparison",
            "The ternary only converts the condition to a boolean, which bool() says directly.",
            "True if x else False becomes bool(x).",
        ),
    ),
    (
        PythonRule::B020,
        RuleText::new(
            "Loop variable that overrides what is being iterated",
            "give the loop variable a different name",
            "Once the loop starts, the name no longer points to the collection, so later uses see the last element.",
            "for items in items: becomes for item in items:.",
        ),
    ),
];
