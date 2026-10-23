use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: modismos de pylint, refurb, flake8-logging y
// compañía.
pub const IDIOM_TEXTS: [(PythonRule, RuleText); 9] = [
    (
        PythonRule::FURB167,
        RuleText::new(
            "Un flag de re con su alias de una letra",
            "usá el nombre completo, como re.IGNORECASE",
            "re.I o re.M obligan a recordar qué significa cada letra; el nombre completo se lee solo.",
            "re.I pasa a re.IGNORECASE.",
        ),
    ),
    (
        PythonRule::PLR2044,
        RuleText::new(
            "Un comentario vacío",
            "borralo, o escribí algo",
            "Un # sin texto no aporta nada y ensucia el código.",
            "x = 1  # pasa a x = 1.",
        ),
    ),
    (
        PythonRule::SIM201,
        RuleText::new(
            "Una igualdad negada con not",
            "usá a != b",
            "La comparación negada se lee directo con !=, sin un not delante.",
            "if not a == b: pasa a if a != b:.",
        ),
    ),
    (
        PythonRule::PLW1510,
        RuleText::new(
            "subprocess.run sin check",
            "pasá check=True, o check=False si el error se maneja a mano",
            "Sin check, un comando que falla no lanza nada y el programa sigue como si hubiera salido bien.",
            "subprocess.run(comando) pasa a subprocess.run(comando, check=True).",
        ),
    ),
    (
        PythonRule::PLR0124,
        RuleText::new(
            "Se compara algo consigo mismo",
            "revisá la comparación: probablemente quería ser con otra cosa",
            "x == x siempre da lo mismo (salvo NaN): casi siempre es un error de tipeo.",
            "if a == a: pasa a if a == b:.",
        ),
    ),
    (
        PythonRule::PLR0133,
        RuleText::new(
            "Se comparan dos constantes",
            "reemplazá la comparación por su resultado, o revisá qué se quería comparar",
            "Comparar dos literales da siempre el mismo resultado: la condición no decide nada.",
            "if 1 == 1: pasa a if True:, o a la comparación que se quería hacer.",
        ),
    ),
    (
        PythonRule::C401,
        RuleText::new(
            "set() sobre un generador",
            "usá una comprensión de conjunto",
            "{x for x in y} hace lo mismo que set(x for x in y), más corto y sin una llamada de más.",
            "set(x for x in y) pasa a {x for x in y}.",
        ),
    ),
    (
        PythonRule::C402,
        RuleText::new(
            "dict() sobre un generador de pares",
            "usá una comprensión de diccionario",
            "{k: v for ...} hace lo mismo que dict((k, v) for ...), más claro y sin armar tuplas intermedias.",
            "dict((k, v) for k, v in pares) pasa a {k: v for k, v in pares}.",
        ),
    ),
    (
        PythonRule::LOG015,
        RuleText::new(
            "Se registra con el logger raíz",
            "usá un logger propio del módulo",
            "logging.info y compañía escriben en el logger raíz: no se puede filtrar ni configurar por módulo.",
            "logging.info(\"x\") pasa a logger = logging.getLogger(__name__) y logger.info(\"x\").",
        ),
    ),
];
