use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas UP, RUF y YTT del conjunto por defecto de ruff 0.17 que no aparecen en el corpus.
pub const UPGRADE_RULE_TEXTS: [(PythonRule, RuleText); 22] = [
    (
        PythonRule::UP001,
        RuleText::new(
            "__metaclass__ = type que no hace nada",
            "borrá la asignación",
            "En Python 3 toda clase ya usa type como metaclase, y __metaclass__ ya no se lee.",
            "class A: __metaclass__ = type pasa a class A: pass.",
        ),
    ),
    (
        PythonRule::UP023,
        RuleText::new(
            "Import del módulo deprecado xml.etree.cElementTree",
            "importá xml.etree.ElementTree",
            "cElementTree era un alias de ElementTree y se sacó en Python 3.9.",
            "import xml.etree.cElementTree as ET pasa a import xml.etree.ElementTree as ET.",
        ),
    ),
    (
        PythonRule::UP026,
        RuleText::new(
            "Uso del paquete mock en vez de unittest.mock",
            "importá mock desde unittest",
            "unittest.mock es parte de la biblioteca estándar, así que la dependencia extra mock no hace falta.",
            "import mock pasa a from unittest import mock.",
        ),
    ),
    (
        PythonRule::RUF008,
        RuleText::new(
            "Valor mutable por defecto en un campo de dataclass",
            "usá field(default_factory=...)",
            "El valor se crea una sola vez y lo comparten todas las instancias, así que un cambio en una aparece en todas.",
            "items: list[int] = [] pasa a items: list[int] = field(default_factory=list).",
        ),
    ),
    (
        PythonRule::RUF016,
        RuleText::new(
            "Índice de un tipo equivocado sobre una lista, tupla o texto literal",
            "indexá con un entero o un corte",
            "Indexar una lista, tupla o texto con cualquier otra cosa tira TypeError.",
            "[1, 2][\"a\"] falla; [1, 2][0] funciona.",
        ),
    ),
    (
        PythonRule::RUF018,
        RuleText::new(
            "Asignación con := adentro de un assert",
            "asigná el valor antes del assert",
            "Los assert desaparecen con python -O, así que la variable nunca se asigna y los usos de después fallan.",
            "assert (x := f()) seguido de print(x) pasa a x = f(); assert x.",
        ),
    ),
    (
        PythonRule::RUF024,
        RuleText::new(
            "Valor mutable pasado a dict.fromkeys",
            "usá una comprensión de diccionario",
            "dict.fromkeys usa el mismo objeto para todas las claves, así que cambiar un valor cambia todos.",
            "dict.fromkeys(keys, []) pasa a {key: [] for key in keys}.",
        ),
    ),
    (
        PythonRule::RUF026,
        RuleText::new(
            "default_factory pasado por nombre a defaultdict",
            "pasá la función como primer argumento posicional",
            "defaultdict recibe la función sólo como argumento posicional; por nombre pasa a ser una clave del diccionario.",
            "defaultdict(default_factory=list) pasa a defaultdict(list).",
        ),
    ),
    (
        PythonRule::RUF030,
        RuleText::new(
            "print() como mensaje de un assert",
            "pasá el mensaje directamente",
            "print corre y devuelve None, así que el error del assert queda sin mensaje.",
            "assert x, print(\"mal\") pasa a assert x, \"mal\".",
        ),
    ),
    (
        PythonRule::RUF033,
        RuleText::new(
            "Parámetro de __post_init__ con valor por defecto",
            "declaralo como campo InitVar con ese valor",
            "La dataclass siempre le pasa cada InitVar a __post_init__, así que el valor por defecto nunca se usa.",
            "def __post_init__(self, extra=1) pasa a extra: InitVar[int] = 1 en la clase.",
        ),
    ),
    (
        PythonRule::RUF048,
        RuleText::new(
            "__version__ parseado con map(int, ...)",
            "usá packaging.version",
            "Una versión como 1.2.3rc1 tiene partes que no son enteros, e int() tira ValueError con ellas.",
            "map(int, __version__.split(\".\")) pasa a packaging.version.parse(__version__).",
        ),
    ),
    (
        PythonRule::RUF049,
        RuleText::new(
            "Enum decorado con @dataclass",
            "sacale el decorador",
            "Los miembros de un enum no se construyen a partir de campos, así que lo que agrega dataclass rompe el enum.",
            "@dataclass en class Color(Enum) se puede borrar.",
        ),
    ),
    (
        PythonRule::RUF058,
        RuleText::new(
            "itertools.starmap sobre un zip",
            "usá map con los iterables directamente",
            "map ya recibe varios iterables en paralelo, así que juntarlos con zip y volver a separarlos es trabajo de más.",
            "starmap(pow, zip(a, b)) pasa a map(pow, a, b).",
        ),
    ),
    (
        PythonRule::RUF101,
        RuleText::new(
            "Código viejo de regla en un noqa",
            "usá el código actual de la regla",
            "El código viejo sigue andando por una redirección, pero esconde qué regla se silencia de verdad.",
            "# noqa: TCH001 pasa a # noqa: TC001.",
        ),
    ),
    (
        PythonRule::YTT101,
        RuleText::new(
            "sys.version[:3] usado como versión",
            "usá sys.version_info",
            "Desde Python 3.10, los tres primeros caracteres son 3.1, no la versión real.",
            "sys.version[:3] pasa a \"{}.{}\".format(*sys.version_info[:2]).",
        ),
    ),
    (
        PythonRule::YTT102,
        RuleText::new(
            "sys.version[2] usado como versión menor",
            "usá sys.version_info.minor",
            "Desde Python 3.10, el tercer carácter es sólo el primer dígito de la versión menor.",
            "sys.version[2] pasa a sys.version_info.minor.",
        ),
    ),
    (
        PythonRule::YTT103,
        RuleText::new(
            "sys.version comparado con un texto",
            "compará sys.version_info con una tupla",
            "Comparar textos compara letra por letra, así que \"3.10\" es menor que \"3.9\".",
            "sys.version >= \"3.5\" pasa a sys.version_info >= (3, 5).",
        ),
    ),
    (
        PythonRule::YTT202,
        RuleText::new(
            "six.PY3 usado para chequear la versión",
            "usá not six.PY2",
            "six.PY3 sólo es verdadero en Python 3, así que se rompe con un futuro Python 4.",
            "if six.PY3: pasa a if not six.PY2:.",
        ),
    ),
    (
        PythonRule::YTT204,
        RuleText::new(
            "sys.version_info.minor comparado con un entero",
            "compará sys.version_info con una tupla",
            "Comparar sólo la versión menor se rompe cuando cambia la mayor.",
            "sys.version_info.minor > 7 pasa a sys.version_info > (3, 7).",
        ),
    ),
    (
        PythonRule::YTT301,
        RuleText::new(
            "sys.version[0] usado como versión mayor",
            "usá sys.version_info.major",
            "Con una versión mayor de dos dígitos, el primer carácter ya no es la versión mayor entera.",
            "sys.version[0] pasa a sys.version_info.major.",
        ),
    ),
    (
        PythonRule::YTT302,
        RuleText::new(
            "sys.version comparado con un texto de un carácter",
            "compará sys.version_info con una tupla",
            "Comparar textos compara letra por letra, así que el chequeo se rompe con una versión mayor de dos dígitos.",
            "sys.version >= \"3\" pasa a sys.version_info >= (3,).",
        ),
    ),
    (
        PythonRule::YTT303,
        RuleText::new(
            "sys.version[:1] usado como versión mayor",
            "usá sys.version_info.major",
            "Con una versión mayor de dos dígitos, el primer carácter ya no es la versión mayor entera.",
            "sys.version[:1] pasa a str(sys.version_info.major).",
        ),
    ),
];
