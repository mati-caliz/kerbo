use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Las reglas del conjunto por defecto de ruff 0.17 fuera del clásico (pyupgrade, flake8-comprehensions
// y compañía).
pub const MODERN_TEXTS: [(PythonRule, RuleText); 2] = [
    (
        PythonRule::C408,
        RuleText::new(
            "dict(), list() or tuple() instead of the literal",
            "use {}, [] or ()",
            "The call is slower than the literal and can be shadowed by another dict or list; the literal says the same without surprises.",
            "dict(a=1) becomes {\"a\": 1} and list() becomes [].",
        ),
    ),
    (
        PythonRule::UP031,
        RuleText::new(
            "% formatting instead of str.format or an f-string",
            "use an f-string or str.format",
            "% formatting has odd cases (a tuple as the only value, mismatched types) and reads worse than an f-string.",
            "\"hello %s\" % name becomes f\"hello {name}\".",
        ),
    ),
];
