use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Las reglas del conjunto por defecto de ruff 0.17 fuera del clásico (pyupgrade, flake8-comprehensions
// y compañía).
pub const MODERN_TEXTS: [(PythonRule, RuleText); 33] = [
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
    (
        PythonRule::UP004,
        RuleText::new(
            "Una clase que hereda de object",
            "sacá object de las bases",
            "En Python 3 toda clase hereda de object: escribirlo no agrega nada.",
            "class Pedido(object): pasa a class Pedido:.",
        ),
    ),
    (
        PythonRule::UP005,
        RuleText::new(
            "Un alias de unittest que se sacó en Python 3.12",
            "usá el nombre nuevo",
            "assertEquals y compañía eran alias viejos: en 3.12 ya no existen y el test falla.",
            "self.assertEquals(a, b) pasa a self.assertEqual(a, b).",
        ),
    ),
    (
        PythonRule::UP009,
        RuleText::new(
            "Una declaración de codificación utf-8",
            "borrala",
            "En Python 3 el código fuente ya es utf-8 por defecto: la línea sobra.",
            "# -*- coding: utf-8 -*- se borra.",
        ),
    ),
    (
        PythonRule::UP025,
        RuleText::new(
            "Un prefijo u en un string",
            "sacá la u",
            "En Python 3 todos los strings son unicode: el prefijo u no cambia nada.",
            "u\"hola\" pasa a \"hola\".",
        ),
    ),
    (
        PythonRule::C405,
        RuleText::new(
            "set() con una lista o tupla literal",
            "usá el literal de set",
            "Construir una lista para convertirla en set es más lento que escribir el set directamente.",
            "set([1, 2]) pasa a {1, 2}.",
        ),
    ),
    (
        PythonRule::B015,
        RuleText::new(
            "Una comparación cuyo resultado se descarta",
            "usala en un assert o un if, o borrala",
            "Una comparación suelta no hace nada: casi siempre falta el assert o el if.",
            "x == 1 pasa a assert x == 1.",
        ),
    ),
    (
        PythonRule::B018,
        RuleText::new(
            "Una expresión suelta que no hace nada",
            "borrala o usá su valor",
            "Una expresión sin efectos como sentencia se calcula y se tira: suele ser un resto de código o un error.",
            "self.cargado se borra o pasa a return self.cargado.",
        ),
    ),
    (
        PythonRule::PIE790,
        RuleText::new(
            "Un pass o ... innecesario",
            "borralo",
            "El bloque ya tiene otra sentencia: el pass o el ... no hacen falta.",
            "def f():\n    \"\"\"Doc.\"\"\"\n    pass pierde el pass.",
        ),
    ),
    (
        PythonRule::UP006,
        RuleText::new(
            "List, Dict y compañía de typing donde alcanza con list, dict",
            "usá el tipo builtin o el de collections",
            "Desde Python 3.9 list[int] funciona igual: el import de typing sobra y se lee peor.",
            "List[int] pasa a list[int] y typing.Dict[str, int] a dict[str, int].",
        ),
    ),
    (
        PythonRule::UP007,
        RuleText::new(
            "Union[X, Y] en vez de X | Y",
            "escribí la unión con |",
            "Desde Python 3.10 la unión con | es más corta y no pide importar nada.",
            "Union[int, str] pasa a int | str.",
        ),
    ),
    (
        PythonRule::UP045,
        RuleText::new(
            "Optional[X] en vez de X | None",
            "escribí X | None",
            "Desde Python 3.10 X | None dice lo mismo sin importar Optional.",
            "Optional[int] pasa a int | None.",
        ),
    ),
    (
        PythonRule::SIM102,
        RuleText::new(
            "Un if cuyo único contenido es otro if",
            "juntá las dos condiciones con and",
            "Dos if anidados sin nada más suman un nivel de sangría que no aporta: una sola condición se lee mejor.",
            "if a:\n    if b: pasa a if a and b:.",
        ),
    ),
    (
        PythonRule::SIM117,
        RuleText::new(
            "Un with cuyo único contenido es otro with",
            "abrí los dos contextos en el mismo with",
            "Un with por contexto suma sangría sin necesidad: with a, b: dice lo mismo.",
            "with abrir(x) as a:\n    with abrir(y) as b: pasa a with abrir(x) as a, abrir(y) as b:.",
        ),
    ),
    (
        PythonRule::BLE001,
        RuleText::new(
            "Un except que atrapa Exception sin volver a lanzarla ni registrarla",
            "atrapá la excepción concreta que esperás, o registrala con logging.exception",
            "Atrapar Exception a ciegas esconde errores de programación y fallas que no se esperaban: el programa sigue como si nada.",
            "except Exception: pass pasa a except ValueError: pass, o a except Exception: logger.exception(\"falló\").",
        ),
    ),
    (
        PythonRule::TRY002,
        RuleText::new(
            "Se lanza Exception o BaseException directamente",
            "definí una excepción propia, o usá una más específica",
            "Quien atrape el error no puede distinguirlo de cualquier otro: le queda atrapar Exception entera.",
            "raise Exception(\"sin saldo\") pasa a raise SaldoInsuficiente(\"sin saldo\").",
        ),
    ),
    (
        PythonRule::PLC0414,
        RuleText::new(
            "Un import con un alias igual al nombre original",
            "sacá el as",
            "import x as x dice lo mismo que import x; sólo tiene sentido en __init__.py y en stubs, para reexportar.",
            "import os as os pasa a import os.",
        ),
    ),
    (
        PythonRule::S102,
        RuleText::new(
            "Uso de exec",
            "evitá exec: resolvé el problema sin ejecutar código armado en tiempo de ejecución",
            "exec corre cualquier código que reciba: si el texto viene de afuera, es una puerta abierta.",
            "exec(codigo) pasa a una función o tabla de despacho que haga lo mismo.",
        ),
    ),
    (
        PythonRule::S110,
        RuleText::new(
            "Un except que atrapa todo y sólo hace pass",
            "registrá el error, o atrapá sólo la excepción que esperás",
            "Un error que se traga en silencio no deja rastro: después nadie sabe por qué el programa hizo otra cosa.",
            "except Exception: pass pasa a except Exception: logger.exception(\"falló\").",
        ),
    ),
    (
        PythonRule::SIM118,
        RuleText::new(
            "Se pregunta por una clave en d.keys()",
            "preguntá directamente en el diccionario",
            "clave in d hace lo mismo que clave in d.keys() sin crear una vista intermedia, y se lee mejor.",
            "if clave in d.keys(): pasa a if clave in d:.",
        ),
    ),
    (
        PythonRule::UP017,
        RuleText::new(
            "Uso de datetime.timezone.utc",
            "usá datetime.UTC",
            "Desde Python 3.11 datetime.UTC es el alias corto para la zona horaria UTC.",
            "datetime.timezone.utc pasa a datetime.UTC.",
        ),
    ),
    (
        PythonRule::UP018,
        RuleText::new(
            "Un str, int, float, bool, bytes o complex que envuelve un literal del mismo tipo",
            "escribí el literal directamente",
            "La llamada no cambia el valor: sólo suma ruido y una llamada de más.",
            "str(\"hola\") pasa a \"hola\", e int() pasa a 0.",
        ),
    ),
    (
        PythonRule::UP030,
        RuleText::new(
            "Un .format con posiciones numeradas en orden",
            "sacá los números de las llaves",
            "\"{0} {1}\" dice lo mismo que \"{} {}\": los números sólo agregan algo que mantener.",
            "\"{0} {1}\".format(a, b) pasa a \"{} {}\".format(a, b).",
        ),
    ),
    (
        PythonRule::UP008,
        RuleText::new(
            "super con la clase y la instancia como argumentos",
            "usá super() sin argumentos",
            "Desde Python 3, super() encuentra la clase y la instancia solo; repetirlas suma algo que se rompe al renombrar la clase.",
            "super(Hija, self).__init__() pasa a super().__init__().",
        ),
    ),
    (
        PythonRule::PIE808,
        RuleText::new(
            "range con un 0 explícito como inicio",
            "sacá el 0: range ya empieza ahí",
            "range(0, n) dice lo mismo que range(n) con más ruido.",
            "range(0, 10) pasa a range(10).",
        ),
    ),
    (
        PythonRule::B009,
        RuleText::new(
            "getattr con un nombre de atributo fijo",
            "accedé al atributo directamente",
            "getattr(x, \"a\") es x.a escrito de forma indirecta: el analizador y el editor pierden de vista el atributo.",
            "getattr(usuario, \"nombre\") pasa a usuario.nombre.",
        ),
    ),
    (
        PythonRule::B010,
        RuleText::new(
            "setattr con un nombre de atributo fijo",
            "asigná el atributo directamente",
            "setattr(x, \"a\", v) es x.a = v escrito de forma indirecta: el analizador y el editor pierden de vista el atributo.",
            "setattr(usuario, \"nombre\", valor) pasa a usuario.nombre = valor.",
        ),
    ),
    (
        PythonRule::RUF015,
        RuleText::new(
            "Se arma una lista o tupla entera sólo para tomar el primer elemento",
            "usá next(iter(...))",
            "list(x)[0] recorre y copia todo el iterable para quedarse con un elemento.",
            "list(usuarios)[0] pasa a next(iter(usuarios)).",
        ),
    ),
    (
        PythonRule::B006,
        RuleText::new(
            "Un parámetro con un valor por defecto mutable",
            "usá None como default y creá el valor adentro de la función",
            "El default se crea una sola vez, al definir la función: todas las llamadas comparten y modifican el mismo objeto.",
            "def agregar(x, lista=[]): pasa a def agregar(x, lista=None): con lista = [] si es None.",
        ),
    ),
    (
        PythonRule::B008,
        RuleText::new(
            "Una llamada en el valor por defecto de un parámetro",
            "calculá el valor adentro de la función, o guardalo en una constante del módulo",
            "El default se evalúa una sola vez, al definir la función, no en cada llamada: un now() queda congelado.",
            "def registrar(momento=datetime.now()): pasa a def registrar(momento=None): con momento = datetime.now() si es None.",
        ),
    ),
    (
        PythonRule::RUF012,
        RuleText::new(
            "Un atributo de clase con un valor mutable",
            "anotalo con ClassVar si es compartido a propósito, o crealo en __init__",
            "El valor se crea una sola vez y lo comparten todas las instancias: modificarlo desde una lo cambia para todas.",
            "items = [] pasa a items: ClassVar[list[str]] = [], o a self.items = [] en __init__.",
        ),
    ),
    (
        PythonRule::RUF059,
        RuleText::new(
            "Una variable desempaquetada que no se usa",
            "renombrala con un _ adelante, o reemplazala por _",
            "Un nombre que se desempaqueta y nunca se lee hace pensar que el valor importa, y esconde errores de tipeo.",
            "nombre, edad = persona con edad sin usar pasa a nombre, _edad = persona.",
        ),
    ),
];
