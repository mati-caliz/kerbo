use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: modismos de pylint, refurb, flake8-logging y
// compañía.
pub const IDIOM_TEXTS: [(PythonRule, RuleText); 34] = [
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
    (
        PythonRule::UP034,
        RuleText::new(
            "Paréntesis de más alrededor de una expresión",
            "sacá los paréntesis duplicados",
            "Un segundo par de paréntesis no agrega nada y hace buscar una tupla o una precedencia que no existe.",
            "print((x)) pasa a print(x).",
        ),
    ),
    (
        PythonRule::PLC0205,
        RuleText::new(
            "__slots__ asignado a un único string",
            "usá una tupla o lista de nombres, como (\"nombre\",)",
            "Un string se recorre letra por letra, así que __slots__ = \"nombre\" declara los slots n, o, m, b, r y e.",
            "__slots__ = \"nombre\" pasa a __slots__ = (\"nombre\",).",
        ),
    ),
    (
        PythonRule::PLW0120,
        RuleText::new(
            "else en un loop que nunca hace break",
            "sacá el else y desindentá su cuerpo",
            "Sin break el else corre siempre, así que se lee como una condición que no existe.",
            "for x in y:
    f(x)
else:
    g() pasa al loop seguido de g().",
        ),
    ),
    (
        PythonRule::RET501,
        RuleText::new(
            "return None en una función que sólo devuelve None",
            "escribí return a secas",
            "Si la función nunca devuelve un valor, un None explícito sugiere un resultado del que no hay que depender.",
            "return None pasa a return.",
        ),
    ),
    (
        PythonRule::PLR1711,
        RuleText::new(
            "return inútil al final de una función",
            "sacá el return",
            "Una función ya devuelve None al llegar al final, así que el último return sólo agrega una línea.",
            "def f():\n    g()\n    return pasa a def f():\n    g().",
        ),
    ),
    (
        PythonRule::PLR0402,
        RuleText::new(
            "import a.b as b en vez de from a import b",
            "escribí from a import b",
            "from a import b dice lo mismo con menos repetición y es la forma que se espera leer.",
            "import os.path as path pasa a from os import path.",
        ),
    ),
    (
        PythonRule::B026,
        RuleText::new(
            "Argumento con * después de un argumento con nombre",
            "poné el *args antes de los argumentos con nombre",
            "Python pasa el *args antes que los argumentos con nombre aunque esté escrito después, y eso confunde el orden.",
            "f(a=1, *resto) pasa a f(*resto, a=1).",
        ),
    ),
    (
        PythonRule::UP039,
        RuleText::new(
            "Paréntesis vacíos en la definición de una clase",
            "sacá los paréntesis",
            "class C(): y class C: son lo mismo; los paréntesis sugieren una clase base que no está.",
            "class C(): pasa a class C:.",
        ),
    ),
    (
        PythonRule::UP020,
        RuleText::new(
            "io.open en vez del open de siempre",
            "usá open",
            "Desde Python 3 el open de siempre es io.open, así que el import no agrega nada.",
            "io.open(\"f\") pasa a open(\"f\").",
        ),
    ),
    (
        PythonRule::RUF032,
        RuleText::new(
            "Decimal construido desde un float escrito",
            "pasá el número como string",
            "El float se redondea antes de que Decimal lo vea, así que Decimal(0.1) no es 0.1.",
            "Decimal(0.1) pasa a Decimal(\"0.1\").",
        ),
    ),
    (
        PythonRule::TRY201,
        RuleText::new(
            "raise error adentro de su propio except",
            "escribí raise a secas",
            "Un raise a secas vuelve a lanzar la misma excepción sin sumar esta línea al traceback.",
            "except ValueError as error:\n    raise error pasa a raise.",
        ),
    ),
    (
        PythonRule::UP012,
        RuleText::new(
            "encode(\"utf-8\") sobre un string fijo",
            "escribí un literal de bytes, o llamá a encode() sin la codificación",
            "UTF-8 ya es la codificación por defecto, y un literal ASCII se puede escribir directamente como bytes.",
            "\"abc\".encode(\"utf-8\") pasa a b\"abc\".",
        ),
    ),
    (
        PythonRule::C400,
        RuleText::new(
            "Generador pasado a list() en vez de una comprensión de lista",
            "escribí una comprensión de lista",
            "Una comprensión arma la lista directamente y se lee como lista de un vistazo.",
            "list(x * 2 for x in y) pasa a [x * 2 for x in y].",
        ),
    ),
    (
        PythonRule::C409,
        RuleText::new(
            "tuple() alrededor de una lista o tupla escrita",
            "escribí la tupla directamente",
            "El literal ya dice que es una tupla sin armar una colección intermedia.",
            "tuple([1, 2]) pasa a (1, 2).",
        ),
    ),
    (
        PythonRule::C413,
        RuleText::new(
            "list() o reversed() alrededor de sorted()",
            "usá sorted() solo, con reverse=True si hace falta",
            "sorted() ya devuelve una lista nueva y puede ordenar al revés por sí mismo.",
            "list(sorted(x)) pasa a sorted(x).",
        ),
    ),
    (
        PythonRule::C419,
        RuleText::new(
            "Comprensión adentro de any() o all()",
            "pasá un generador",
            "Con un generador, any() y all() cortan en el primer elemento que decide en vez de armar la lista entera.",
            "any([x > 0 for x in y]) pasa a any(x > 0 for x in y).",
        ),
    ),
];
