use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Las reglas del conjunto por defecto de ruff 0.17 fuera del clásico (pyupgrade, flake8-comprehensions
// y compañía).
pub const MODERN_TEXTS: [(PythonRule, RuleText); 2] = [
    (
        PythonRule::C408,
        RuleText::new(
            "dict(), list() o tuple() en vez del literal",
            "usá {}, [] o ()",
            "La llamada es más lenta que el literal y se puede tapar con otro dict o list; el literal dice lo mismo sin sorpresas.",
            "dict(a=1) pasa a {\"a\": 1} y list() a [].",
        ),
    ),
    (
        PythonRule::UP031,
        RuleText::new(
            "Formato con % en vez de str.format o f-string",
            "usá una f-string o str.format",
            "El formato con % tiene casos raros (una tupla como único valor, tipos que no coinciden) y se lee peor que una f-string.",
            "\"hola %s\" % nombre pasa a f\"hola {nombre}\".",
        ),
    ),
];
