use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas de pylint del conjunto por defecto de ruff 0.17.
pub const PYLINT_ERROR_TEXTS: [(PythonRule, RuleText); 20] = [
    (
        PythonRule::PLE0100,
        RuleText::new(
            "__init__ que es un generador",
            "llevá el yield a otro método",
            "Un yield convierte a __init__ en un generador, y Python tira TypeError al instanciar la clase.",
            "def __init__(self): yield 1 falla en Foo().",
        ),
    ),
    (
        PythonRule::PLE0115,
        RuleText::new(
            "Nombre declarado nonlocal y global a la vez",
            "dejá una sola de las declaraciones",
            "Un nombre no puede ser nonlocal y global en el mismo alcance; Python rechaza la función.",
            "global x seguido de nonlocal x es un SyntaxError.",
        ),
    ),
    (
        PythonRule::PLE0118,
        RuleText::new(
            "Nombre usado antes de su declaración global",
            "subí el global antes del primer uso",
            "Python exige que la declaración global venga antes de cualquier uso del nombre en ese alcance.",
            "print(x) seguido de global x es un SyntaxError.",
        ),
    ),
    (
        PythonRule::PLE1132,
        RuleText::new(
            "Argumento con nombre repetido en un diccionario con **",
            "sacá una de las claves repetidas",
            "Python tira TypeError cuando el mismo argumento con nombre llega dos veces a una llamada.",
            "f(a=1, **{\"a\": 2}) falla con varios valores para a.",
        ),
    ),
    (
        PythonRule::PLE1205,
        RuleText::new(
            "Llamada de logging con más argumentos que marcadores",
            "agregá los marcadores que faltan o sacá los argumentos de más",
            "El módulo logging informa un error de formato en vez del mensaje cuando sobran argumentos.",
            "logging.info(\"%s\", a, b) tiene un argumento de más.",
        ),
    ),
    (
        PythonRule::PLE1206,
        RuleText::new(
            "Llamada de logging con menos argumentos que marcadores",
            "pasá un argumento por marcador",
            "El módulo logging informa un error de formato en vez del mensaje cuando faltan argumentos.",
            "logging.info(\"%s %s\", a) tiene un argumento de menos.",
        ),
    ),
    (
        PythonRule::PLE1310,
        RuleText::new(
            "strip con caracteres repetidos",
            "sacá los caracteres repetidos, o usá removeprefix o removesuffix",
            "strip recibe un conjunto de caracteres, no un prefijo, así que repetir uno no cambia nada y suele esconder un error.",
            "name.lstrip(\"abca\") saca a, b y c en cualquier orden; name.removeprefix(\"abca\") saca el texto.",
        ),
    ),
    (
        PythonRule::PLE1507,
        RuleText::new(
            "os.getenv con una clave que no es texto",
            "pasá el nombre de la variable como texto",
            "os.getenv sólo acepta texto como nombre; cualquier otra cosa tira TypeError.",
            "os.getenv(1) pasa a os.getenv(\"1\").",
        ),
    ),
    (
        PythonRule::PLE1519,
        RuleText::new(
            "singledispatch en un método",
            "usá functools.singledispatchmethod",
            "singledispatch elige por el primer argumento, que en un método es self, así que nunca mira el argumento real.",
            "@singledispatch en def method(self, arg) pasa a @singledispatchmethod.",
        ),
    ),
    (
        PythonRule::PLE1520,
        RuleText::new(
            "singledispatchmethod en una función suelta",
            "usá functools.singledispatch",
            "singledispatchmethod saltea el primer argumento como si fuera self, así que en una función elige por el equivocado.",
            "@singledispatchmethod en def handle(arg) pasa a @singledispatch.",
        ),
    ),
    (
        PythonRule::PLE1700,
        RuleText::new(
            "yield from adentro de una función async",
            "recorré con un for y hacé yield de cada valor",
            "Python no permite yield from en una función async; el módulo ni siquiera compila.",
            "async def f(): yield from items pasa a for item in items: yield item.",
        ),
    ),
    (
        PythonRule::PLE2510,
        RuleText::new(
            "Carácter de retroceso escrito tal cual en un string",
            "escribilo como \\b",
            "Un carácter de control invisible se pasa por alto al leer el código y puede romper terminales.",
            "\"a<BS>b\" pasa a \"a\\bb\".",
        ),
    ),
    (
        PythonRule::PLE2512,
        RuleText::new(
            "Carácter SUB escrito tal cual en un string",
            "escribilo como \\x1a",
            "Un carácter de control invisible se pasa por alto al leer el código, y algunas herramientas toman SUB como fin de archivo.",
            "\"a<SUB>b\" pasa a \"a\\x1ab\".",
        ),
    ),
    (
        PythonRule::PLE2513,
        RuleText::new(
            "Carácter ESC escrito tal cual en un string",
            "escribilo como \\x1b",
            "Un carácter de escape invisible puede cambiar lo que muestra una terminal cuando se imprime el código.",
            "\"a<ESC>b\" pasa a \"a\\x1bb\".",
        ),
    ),
    (
        PythonRule::PLE2514,
        RuleText::new(
            "Carácter NUL escrito tal cual en un string",
            "escribilo como \\0",
            "Un byte NUL crudo en el código rompe muchas herramientas que leen el archivo como texto.",
            "\"a<NUL>b\" pasa a \"a\\0b\".",
        ),
    ),
    (
        PythonRule::PLE2515,
        RuleText::new(
            "Espacio de ancho cero escrito tal cual en un string",
            "escribilo como \\u200b",
            "Un espacio de ancho cero no se ve, así que dos strings que parecen iguales pueden ser distintos.",
            "\"a<ZWSP>b\" pasa a \"a\\u200bb\".",
        ),
    ),
    (
        PythonRule::PLW0604,
        RuleText::new(
            "Sentencia global en el módulo",
            "borrala",
            "En el módulo todos los nombres ya son globales, así que la sentencia no hace nada.",
            "global counter arriba de todo de un módulo se puede borrar.",
        ),
    ),
    (
        PythonRule::PLW1501,
        RuleText::new(
            "Modo inválido para open",
            "usá una combinación válida de r, w, x, a, b, t y +",
            "open tira ValueError con un modo de letras repetidas, texto y binario juntos, o más de uno de r, w, x y a.",
            "open(path, \"rw\") pasa a open(path, \"r+\").",
        ),
    ),
    (
        PythonRule::PLW1507,
        RuleText::new(
            "Copia superficial de os.environ",
            "usá os.environ.copy()",
            "copy.copy(os.environ) sigue escribiendo en el entorno real, así que los cambios a la copia llegan al proceso.",
            "env = copy.copy(os.environ) pasa a env = os.environ.copy().",
        ),
    ),
    (
        PythonRule::PLW2101,
        RuleText::new(
            "Candado nuevo usado como contexto de un with",
            "creá el candado una vez y compartilo",
            "Un candado creado en el mismo with no lo comparte nadie, así que nunca bloquea a nadie.",
            "with threading.Lock(): pasa a with self.lock: con el candado creado en __init__.",
        ),
    ),
];
