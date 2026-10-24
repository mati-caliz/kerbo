use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: chequeos de excepciones, loops y llamadas.
pub const CHECK_TEXTS: [(PythonRule, RuleText); 4] = [
    (
        PythonRule::TRY203,
        RuleText::new(
            "try cuyos except sólo vuelven a lanzar",
            "sacá el try, o manejá el error en el except",
            "Un except que sólo vuelve a lanzar la misma excepción no hace nada más que sumar indentación.",
            "try:\n    f()\nexcept ValueError:\n    raise pasa a f().",
        ),
    ),
    (
        PythonRule::PLE0704,
        RuleText::new(
            "raise a secas fuera de un except",
            "lanzá una excepción concreta",
            "Un raise a secas fuera de un except no tiene una excepción activa para relanzar y falla con RuntimeError.",
            "raise pasa a raise ValueError(\"...\").",
        ),
    ),
    (
        PythonRule::SIM210,
        RuleText::new(
            "True if condición else False",
            "usá bool(condición), o la condición sola si es una comparación",
            "El ternario sólo convierte la condición a booleano, que es lo que bool() dice directamente.",
            "True if x else False pasa a bool(x).",
        ),
    ),
    (
        PythonRule::B020,
        RuleText::new(
            "Variable del loop que pisa lo que se recorre",
            "dale otro nombre a la variable del loop",
            "Apenas arranca el loop, el nombre deja de apuntar a la colección, así que los usos posteriores ven el último elemento.",
            "for items in items: pasa a for item in items:.",
        ),
    ),
];
