use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: modismos de pylint, refurb, flake8-logging y
// compañía.
pub const IDIOM_TEXTS: [(PythonRule, RuleText); 18] = [
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
    (
        PythonRule::FLY002,
        RuleText::new(
            "Un join sobre una lista fija de elementos",
            "usá una f-string",
            "\" \".join([a, b]) arma una lista para pegar dos cosas que una f-string escribe directo.",
            "\" \".join([nombre, apellido]) pasa a f\"{nombre} {apellido}\".",
        ),
    ),
    (
        PythonRule::PIE804,
        RuleText::new(
            "Un diccionario literal desarmado con ** en una llamada",
            "pasá los argumentos por nombre",
            "f(**{\"a\": 1}) arma un diccionario para decir lo mismo que f(a=1).",
            "f(**{\"a\": 1}) pasa a f(a=1).",
        ),
    ),
    (
        PythonRule::RUF010,
        RuleText::new(
            "str(), repr() o ascii() dentro de una f-string",
            "usá la conversión de la f-string: !s, !r o !a",
            "La f-string ya sabe convertir el valor; la llamada explícita es más larga y más lenta.",
            "f\"{repr(x)}\" pasa a f\"{x!r}\".",
        ),
    ),
    (
        PythonRule::I001,
        RuleText::new(
            "El bloque de imports no está ordenado o formateado",
            "ordenalo por secciones (estándar, terceros, del proyecto, relativos) y alfabéticamente, con una línea en blanco entre secciones",
            "Un orden fijo de imports se revisa de un vistazo, evita duplicados y elimina los conflictos de merge por reordenamientos.",
            "import sys\nimport os pasa a import os\nimport sys.",
        ),
    ),
    (
        PythonRule::UP032,
        RuleText::new(
            "Llamada a str.format que puede ser una f-string",
            "escribila como f-string",
            "Una f-string pone cada valor donde se imprime: se lee de una pasada y no puede emparejar un hueco con el argumento equivocado.",
            "\"hola {}\".format(nombre) pasa a f\"hola {nombre}\".",
        ),
    ),
    (
        PythonRule::N999,
        RuleText::new(
            "El nombre del módulo no es un identificador válido en minúsculas",
            "renombrá el archivo en minúsculas, con guiones bajos y sin empezar con un dígito",
            "Un módulo cuyo nombre no es un identificador no se puede importar con un import común, y PEP 8 pide nombres cortos en minúsculas.",
            "MiModulo.py pasa a mi_modulo.py.",
        ),
    ),
    (
        PythonRule::SIM103,
        RuleText::new(
            "if que devuelve True o False en vez de devolver la condición",
            "devolvé la condición directamente (con bool() o not si hace falta)",
            "Devolver la condición dice lo mismo en una línea y no deja ramas para leer.",
            "if x > 0:
    return True
return False pasa a return x > 0.",
        ),
    ),
    (
        PythonRule::SIM905,
        RuleText::new(
            "split() sobre un string fijo en vez de escribir la lista",
            "escribí la lista que resulta",
            "La lista se ve tal cual, sin calcularla en cada ejecución ni adivinar dónde corta.",
            "\"a b c\".split() pasa a [\"a\", \"b\", \"c\"].",
        ),
    ),
    (
        PythonRule::FURB157,
        RuleText::new(
            "Decimal construido desde un string que podría ser un literal",
            "pasá el número directamente, o el valor especial como string",
            "Decimal(10) dice lo mismo que Decimal(\"10\") sin parsear un string, y Decimal(\"nan\") evita pasar por un float.",
            "Decimal(\"10\") pasa a Decimal(10).",
        ),
    ),
];
