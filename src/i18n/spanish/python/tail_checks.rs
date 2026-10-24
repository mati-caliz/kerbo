use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: loops, llamadas, async y versiones.
pub const TAIL_CHECK_TEXTS: [(PythonRule, RuleText); 27] = [
    (
        PythonRule::UP028,
        RuleText::new(
            "Loop for que sólo cede sus elementos",
            "usá yield from",
            "yield from dice lo mismo en una línea y además pasa send() y throw() al iterador de adentro.",
            "for x in items: yield x pasa a yield from items.",
        ),
    ),
    (
        PythonRule::PIE810,
        RuleText::new(
            "Varios startswith o endswith sobre el mismo valor",
            "pasale una tupla a una sola llamada",
            "startswith y endswith aceptan una tupla de prefijos, así que una sola llamada dice lo mismo.",
            "s.startswith(\"a\") or s.startswith(\"b\") pasa a s.startswith((\"a\", \"b\")).",
        ),
    ),
    (
        PythonRule::C417,
        RuleText::new(
            "map() con una lambda",
            "escribí una comprensión o un generador",
            "Una comprensión se lee más directo que map con una lambda y se ahorra una llamada por elemento.",
            "map(lambda x: x + 1, items) pasa a (x + 1 for x in items).",
        ),
    ),
    (
        PythonRule::PLC0132,
        RuleText::new(
            "Nombre de TypeVar distinto de su variable",
            "usá el mismo nombre en los dos lados",
            "Los chequeadores de tipos y los mensajes de error muestran el nombre que se le pasa a TypeVar, así que uno distinto confunde.",
            "T = TypeVar(\"U\") pasa a T = TypeVar(\"T\").",
        ),
    ),
    (
        PythonRule::YTT203,
        RuleText::new(
            "sys.version_info[1] comparado con un entero",
            "compará sys.version_info con una tupla",
            "Comparar sólo la versión menor se rompe con Python 4, donde el número menor vuelve a empezar.",
            "sys.version_info[1] < 7 pasa a sys.version_info < (3, 7).",
        ),
    ),
    (
        PythonRule::YTT201,
        RuleText::new(
            "Comparación sys.version_info[0] == 3",
            "compará con >= en vez de ==",
            "Con Python 4 la comparación da falso y el código toma la rama de Python 2.",
            "sys.version_info[0] == 3 pasa a sys.version_info >= (3,).",
        ),
    ),
    (
        PythonRule::RUF040,
        RuleText::new(
            "Mensaje de assert que no es texto",
            "escribí el mensaje como texto, o compará adentro del assert",
            "La segunda parte de un assert es el mensaje; un número ahí suele ser una comparación que se quiso escribir.",
            "assert len(items), 2 pasa a assert len(items) == 2.",
        ),
    ),
    (
        PythonRule::PLR1733,
        RuleText::new(
            "Búsqueda en el dict de un valor que el loop ya tiene",
            "usá la variable del valor",
            "Recorrer d.items() ya da el valor, así que d[clave] lo vuelve a buscar sin necesidad.",
            "for k, v in d.items(): print(d[k]) pasa a print(v).",
        ),
    ),
    (
        PythonRule::PLR1736,
        RuleText::new(
            "Búsqueda en la lista de un elemento que el loop ya tiene",
            "usá la variable del elemento",
            "Recorrer enumerate(items) ya da el elemento, así que items[indice] lo vuelve a buscar sin necesidad.",
            "for i, x in enumerate(items): print(items[i]) pasa a print(x).",
        ),
    ),
    (
        PythonRule::FURB136,
        RuleText::new(
            "Ternario que es un min() o un max()",
            "usá min() o max()",
            "Un ternario que elige el menor o el mayor de dos valores dice lo mismo que min() o max().",
            "a if a > b else b pasa a max(a, b).",
        ),
    ),
    (
        PythonRule::PLR1708,
        RuleText::new(
            "raise StopIteration en un generador",
            "usá return",
            "Desde PEP 479 un StopIteration lanzado adentro de un generador se convierte en RuntimeError.",
            "raise StopIteration pasa a return.",
        ),
    ),
    (
        PythonRule::FURB132,
        RuleText::new(
            "Chequeo antes de sacar de un conjunto",
            "usá set.discard",
            "discard saca el elemento si está sin lanzar error, así que el chequeo sobra.",
            "if x in s: s.remove(x) pasa a s.discard(x).",
        ),
    ),
    (
        PythonRule::UP050,
        RuleText::new(
            "metaclass=type en una clase",
            "sacá el argumento metaclass",
            "type ya es la metaclase por defecto.",
            "class A(metaclass=type): pasa a class A:.",
        ),
    ),
    (
        PythonRule::RUF020,
        RuleText::new(
            "Never o NoReturn en una unión",
            "sacalo de la unión",
            "Never no tiene valores, así que no suma nada a una unión.",
            "Never | int pasa a int.",
        ),
    ),
    (
        PythonRule::TRY401,
        RuleText::new(
            "Excepción pasada otra vez a logger.exception",
            "sacala del mensaje",
            "logger.exception ya registra la excepción actual con su traceback.",
            "logger.exception(f\"failed: {e}\") pasa a logger.exception(\"failed\").",
        ),
    ),
    (
        PythonRule::FURB161,
        RuleText::new(
            "bin(x).count(\"1\") para contar bits",
            "usá int.bit_count()",
            "Desde Python 3.10 los enteros cuentan sus bits encendidos directo, sin armar un string.",
            "bin(x).count(\"1\") pasa a x.bit_count().",
        ),
    ),
    (
        PythonRule::FURB168,
        RuleText::new(
            "Chequeo isinstance(x, type(None))",
            "escribí x is None",
            "None tiene una sola instancia, así que comparar la identidad dice lo mismo más directo.",
            "isinstance(x, type(None)) pasa a x is None.",
        ),
    ),
    (
        PythonRule::PLE0117,
        RuleText::new(
            "nonlocal sin una variable en la función de afuera",
            "ligá el nombre en la función de afuera o usá global",
            "Python rechaza un nonlocal que ninguna función de afuera define.",
            "nonlocal count sin count en la función de afuera necesita count = 0 ahí.",
        ),
    ),
    (
        PythonRule::LOG001,
        RuleText::new(
            "logging.Logger creado directo",
            "usá logging.getLogger",
            "Un Logger creado directo no queda registrado, así que ignora la configuración de logging.",
            "logging.Logger(__name__) pasa a logging.getLogger(__name__).",
        ),
    ),
    (
        PythonRule::FURB163,
        RuleText::new(
            "math.log con base 2, 10 o e",
            "usá math.log2, math.log10 o math.log",
            "Las funciones propias son más precisas y dicen la base en el nombre.",
            "math.log(x, 2) pasa a math.log2(x).",
        ),
    ),
    (
        PythonRule::DTZ004,
        RuleText::new(
            "Llamada a datetime.utcfromtimestamp()",
            "usá datetime.fromtimestamp(ts, tz=timezone.utc)",
            "utcfromtimestamp devuelve una fecha sin zona que muchas funciones toman como hora local; está obsoleta desde 3.12.",
            "datetime.utcfromtimestamp(ts) pasa a datetime.fromtimestamp(ts, tz=timezone.utc).",
        ),
    ),
    (
        PythonRule::PERF101,
        RuleText::new(
            "list() de algo que ya se puede recorrer",
            "recorrelo directo",
            "Copiar a una lista antes del loop sólo reserva memoria de más.",
            "for x in list((1, 2)): pasa a for x in (1, 2):.",
        ),
    ),
    (
        PythonRule::B032,
        RuleText::new(
            "Anotación donde se quiso asignar",
            "usá = en vez de :",
            "a[\"b\"]: 1 sólo anota la expresión y no asigna nada.",
            "a[\"b\"]: 1 pasa a a[\"b\"] = 1.",
        ),
    ),
    (
        PythonRule::PLW0711,
        RuleText::new(
            "except con una operación booleana",
            "atrapá una tupla de excepciones",
            "A or B vale A, así que except A or B sólo atrapa A.",
            "except A or B: pasa a except (A, B):.",
        ),
    ),
    (
        PythonRule::ASYNC251,
        RuleText::new(
            "time.sleep en una función async",
            "esperá asyncio.sleep",
            "time.sleep bloquea el loop de eventos y congela todas las otras tareas mientras espera.",
            "time.sleep(1) pasa a await asyncio.sleep(1).",
        ),
    ),
    (
        PythonRule::ASYNC230,
        RuleText::new(
            "open() que bloquea en una función async",
            "usá una API de archivos async como anyio.open_file",
            "Leer o escribir un archivo con open() bloquea el loop de eventos.",
            "with open(path) pasa a async with await anyio.open_file(path).",
        ),
    ),
    (
        PythonRule::ASYNC115,
        RuleText::new(
            "sleep(0) para cederle el turno al loop",
            "usá la función checkpoint de la biblioteca",
            "trio y anyio tienen un checkpoint propio para esto, que dice la intención.",
            "await trio.sleep(0) pasa a await trio.lowlevel.checkpoint().",
        ),
    ),
];
