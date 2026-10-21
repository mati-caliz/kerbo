use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

const RUNTIME_WHY: &str = "La línea falla recién al ejecutarse, con TypeError, KeyError o ValueError.";
const PERCENT_EXAMPLE: &str = "\"%s y %s\" % (a,) pasa a \"%s y %s\" % (a, b), o mejor a una f-string.";
const FORMAT_EXAMPLE: &str = "\"{} y {}\".format(a) pasa a \"{} y {}\".format(a, b), o mejor a una f-string.";

pub const FORMAT_TEXTS: [(PythonRule, RuleText); 14] = [
    (
        PythonRule::F501,
        RuleText::new(
            "String de formato con % inválido",
            "corregí el especificador o escapá el % como %%",
            RUNTIME_WHY,
            "\"100%\" % valor pasa a \"100%%\" % valor.",
        ),
    ),
    (
        PythonRule::F502,
        RuleText::new(
            "El formato con % espera un diccionario y recibe una secuencia",
            "pasá un diccionario con las claves que usa el formato",
            RUNTIME_WHY,
            "\"%(nombre)s\" % (nombre,) pasa a \"%(nombre)s\" % {\"nombre\": nombre}.",
        ),
    ),
    (
        PythonRule::F503,
        RuleText::new(
            "El formato con % espera una secuencia y recibe un diccionario",
            "pasá una tupla con un valor por especificador",
            RUNTIME_WHY,
            PERCENT_EXAMPLE,
        ),
    ),
    (
        PythonRule::F504,
        RuleText::new(
            "Claves del diccionario que el formato con % no usa",
            "sacá las claves de más o usalas en el formato",
            "Una clave que nadie lee suele ser un error de tipeo en el formato.",
            "\"%(a)s\" % {\"a\": 1, \"b\": 2} pasa a \"%(a)s\" % {\"a\": 1}.",
        ),
    ),
    (
        PythonRule::F505,
        RuleText::new(
            "Al diccionario le faltan claves que pide el formato con %",
            "agregá las claves que faltan",
            RUNTIME_WHY,
            "\"%(a)s %(b)s\" % {\"a\": 1} pasa a \"%(a)s %(b)s\" % {\"a\": 1, \"b\": 2}.",
        ),
    ),
    (
        PythonRule::F506,
        RuleText::new(
            "El formato con % mezcla especificadores con y sin nombre",
            "usá todos con nombre o todos posicionales",
            RUNTIME_WHY,
            "\"%s %(a)s\" pasa a \"%(b)s %(a)s\".",
        ),
    ),
    (
        PythonRule::F507,
        RuleText::new(
            "El formato con % recibe otra cantidad de valores que los que pide",
            "pasá un valor por especificador",
            RUNTIME_WHY,
            PERCENT_EXAMPLE,
        ),
    ),
    (
        PythonRule::F508,
        RuleText::new(
            "El * del formato con % necesita una secuencia",
            "pasá una tupla con el ancho o la precisión antes del valor",
            RUNTIME_WHY,
            "\"%*d\" % {\"a\": 1} pasa a \"%*d\" % (5, 1).",
        ),
    ),
    (
        PythonRule::F509,
        RuleText::new(
            "Carácter de conversión que el formato con % no soporta",
            "usá una conversión válida (s, r, d, f...)",
            RUNTIME_WHY,
            "\"%y\" % valor pasa a \"%s\" % valor.",
        ),
    ),
    (
        PythonRule::F521,
        RuleText::new(
            "String de .format inválido",
            "corregí las llaves o escapalas como {{ y }}",
            RUNTIME_WHY,
            "\"{\".format(a) pasa a \"{{\".format().",
        ),
    ),
    (
        PythonRule::F522,
        RuleText::new(
            "Argumentos con nombre que .format no usa",
            "sacá los argumentos de más o usalos en el formato",
            "Un argumento que nadie lee suele ser un error de tipeo en el formato.",
            "\"{a}\".format(a=1, b=2) pasa a \"{a}\".format(a=1).",
        ),
    ),
    (
        PythonRule::F523,
        RuleText::new(
            "Argumentos posicionales que .format no usa",
            "sacá los argumentos de más o usalos en el formato",
            "Un argumento que nadie lee suele ser un error en el formato.",
            "\"{0}\".format(1, 2) pasa a \"{0}\".format(1).",
        ),
    ),
    (
        PythonRule::F524,
        RuleText::new(
            "A .format le faltan argumentos que pide el formato",
            "agregá los argumentos que faltan",
            RUNTIME_WHY,
            FORMAT_EXAMPLE,
        ),
    ),
    (
        PythonRule::F525,
        RuleText::new(
            ".format mezcla campos numerados y automáticos",
            "numerá todos los campos o ninguno",
            RUNTIME_WHY,
            "\"{0} {}\" pasa a \"{0} {1}\".",
        ),
    ),
];
