use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: métodos especiales, conversiones y constantes.
pub const SMALL_CHECK_TEXTS: [(PythonRule, RuleText); 30] = [
    (
        PythonRule::PLR1730,
        RuleText::new(
            "if que es un min() o un max()",
            "usá min() o max()",
            "Un if que se queda con el menor o el mayor de dos valores dice lo mismo que min() o max(), en más líneas.",
            "if x > top: top = x pasa a top = max(top, x).",
        ),
    ),
    (
        PythonRule::PERF102,
        RuleText::new(
            "dict.items() que descarta la clave o el valor",
            "recorré dict.keys() o dict.values()",
            "Armar cada par clave y valor para tirar la mitad es más lento y esconde qué necesita el loop.",
            "for _, value in d.items(): pasa a for value in d.values():.",
        ),
    ),
    (
        PythonRule::C414,
        RuleText::new(
            "Conversión doble innecesaria",
            "sacá la llamada de adentro",
            "La llamada de afuera ya arma una lista, tupla, conjunto o copia ordenada nueva, así que la de adentro es trabajo de más.",
            "list(tuple(items)) pasa a list(items).",
        ),
    ),
    (
        PythonRule::C403,
        RuleText::new(
            "set() de una comprensión de lista",
            "escribí una comprensión de conjunto",
            "Una comprensión de conjunto arma el conjunto directo, sin una lista en el medio.",
            "set([x for x in items]) pasa a {x for x in items}.",
        ),
    ),
    (
        PythonRule::C404,
        RuleText::new(
            "dict() de una comprensión de pares",
            "escribí una comprensión de diccionario",
            "Una comprensión de diccionario arma el diccionario directo, sin una lista de tuplas en el medio.",
            "dict([(k, v) for k, v in pairs]) pasa a {k: v for k, v in pairs}.",
        ),
    ),
    (
        PythonRule::C411,
        RuleText::new(
            "list() de una comprensión de lista",
            "sacá la llamada a list()",
            "La comprensión ya arma una lista nueva.",
            "list([x for x in items]) pasa a [x for x in items].",
        ),
    ),
    (
        PythonRule::C418,
        RuleText::new(
            "dict() de un diccionario escrito",
            "sacá la llamada a dict()",
            "El literal o la comprensión ya arma un diccionario nuevo.",
            "dict({\"a\": 1}) pasa a {\"a\": 1}.",
        ),
    ),
    (
        PythonRule::PLW1508,
        RuleText::new(
            "Default de os.getenv que no es texto",
            "pasá el default como texto",
            "os.getenv devuelve texto cuando la variable existe, así que un default de otro tipo deja el resultado inconsistente.",
            "os.getenv(\"PORT\", 80) pasa a os.getenv(\"PORT\", \"80\").",
        ),
    ),
    (
        PythonRule::RUF007,
        RuleText::new(
            "zip() sobre pares consecutivos",
            "usá itertools.pairwise",
            "itertools.pairwise dice directo que el loop recorre pares consecutivos.",
            "zip(items, items[1:]) pasa a itertools.pairwise(items).",
        ),
    ),
    (
        PythonRule::PLC0105,
        RuleText::new(
            "Nombre de TypeVar que no coincide con su varianza",
            "sumá _co o _contra, o sacalo",
            "PEP 484 nombra con _co las variables de tipo covariantes y con _contra las contravariantes, así se sabe la varianza por el nombre.",
            "T = TypeVar(\"T\", covariant=True) pasa a T_co = TypeVar(\"T_co\", covariant=True).",
        ),
    ),
    (
        PythonRule::SIM222,
        RuleText::new(
            "or con un operando siempre verdadero",
            "sacá lo que nunca se ejecuta",
            "El or se corta en el primer valor verdadero, así que lo que sigue a la constante nunca se ejecuta.",
            "x or True or y pasa a x or True.",
        ),
    ),
    (
        PythonRule::SIM223,
        RuleText::new(
            "and con un operando siempre falso",
            "sacá lo que nunca se ejecuta",
            "El and se corta en el primer valor falso, así que lo que sigue a la constante nunca se ejecuta.",
            "x and False and y pasa a x and False.",
        ),
    ),
    (
        PythonRule::SIM208,
        RuleText::new(
            "Doble negación",
            "sacá los dos not, o usá bool()",
            "not not x es lo mismo que bool(x), y que x a secas en una condición.",
            "if not not x: pasa a if x:.",
        ),
    ),
    (
        PythonRule::SIM211,
        RuleText::new(
            "Ternario False if x else True",
            "escribí not x",
            "El ternario sólo niega la condición.",
            "False if x else True pasa a not x.",
        ),
    ),
    (
        PythonRule::RUF034,
        RuleText::new(
            "Ternario con el mismo valor en las dos ramas",
            "dejá sólo el valor",
            "Las dos ramas dan lo mismo, así que la condición no hace nada.",
            "\"a\" if x else \"a\" pasa a \"a\".",
        ),
    ),
    (
        PythonRule::B030,
        RuleText::new(
            "except con algo que no es una clase de excepción",
            "atrapá clases de excepción",
            "Atrapar un valor que no es una clase derivada de BaseException lanza TypeError cuando llega una excepción.",
            "except 1: pasa a except ValueError:.",
        ),
    ),
    (
        PythonRule::B016,
        RuleText::new(
            "raise de un literal",
            "lanzá una instancia de excepción",
            "Lanzar un literal es un TypeError: sólo se pueden lanzar excepciones.",
            "raise \"boom\" pasa a raise RuntimeError(\"boom\").",
        ),
    ),
    (
        PythonRule::PLW0129,
        RuleText::new(
            "assert sobre un string escrito",
            "afirmá una condición de verdad",
            "Un string con texto siempre es verdadero y uno vacío siempre falso, así que el assert nunca chequea nada.",
            "assert \"ready\" pasa a assert ready, \"not ready\".",
        ),
    ),
    (
        PythonRule::PLW0177,
        RuleText::new(
            "Comparación con NaN",
            "usá math.isnan",
            "NaN no es igual a nada, ni a sí mismo, así que la comparación siempre da lo mismo.",
            "x == float(\"nan\") pasa a math.isnan(x).",
        ),
    ),
    (
        PythonRule::PLE0101,
        RuleText::new(
            "__init__ que devuelve un valor",
            "sacá el valor devuelto",
            "__init__ tiene que devolver None; devolver otra cosa lanza TypeError al crear el objeto.",
            "return self.value dentro de __init__ pasa a return.",
        ),
    ),
    (
        PythonRule::PIE800,
        RuleText::new(
            "** innecesario de un diccionario escrito",
            "escribí las claves directo",
            "Desparramar un diccionario escrito dentro de otro sólo agrega ruido.",
            "{**base, **{\"a\": 1}} pasa a {**base, \"a\": 1}.",
        ),
    ),
    (
        PythonRule::PLE0303,
        RuleText::new(
            "__len__ que no devuelve un entero no negativo",
            "devolvé un entero no negativo",
            "len() lanza TypeError o ValueError cuando __len__ devuelve otra cosa.",
            "return \"2\" dentro de __len__ pasa a return 2.",
        ),
    ),
    (
        PythonRule::PLE0304,
        RuleText::new(
            "__bool__ que no devuelve un bool",
            "devolvé True o False",
            "bool() lanza TypeError cuando __bool__ devuelve algo que no es un bool.",
            "return 1 dentro de __bool__ pasa a return True.",
        ),
    ),
    (
        PythonRule::PLE0305,
        RuleText::new(
            "__index__ que no devuelve un int",
            "devolvé un entero",
            "Python lanza TypeError cuando __index__ devuelve algo que no es un int.",
            "return \"1\" dentro de __index__ pasa a return 1.",
        ),
    ),
    (
        PythonRule::PLE0307,
        RuleText::new(
            "__str__ que no devuelve un str",
            "devolvé un string",
            "str() lanza TypeError cuando __str__ devuelve algo que no es un str.",
            "return 3 dentro de __str__ pasa a return \"3\".",
        ),
    ),
    (
        PythonRule::PLE0308,
        RuleText::new(
            "__bytes__ que no devuelve bytes",
            "devolvé un objeto bytes",
            "bytes() lanza TypeError cuando __bytes__ devuelve algo que no es bytes.",
            "return \"text\" dentro de __bytes__ pasa a return b\"text\".",
        ),
    ),
    (
        PythonRule::PLE0309,
        RuleText::new(
            "__hash__ que no devuelve un int",
            "devolvé un entero",
            "hash() lanza TypeError cuando __hash__ devuelve algo que no es un int.",
            "return 1.5 dentro de __hash__ pasa a return hash(self.value).",
        ),
    ),
    (
        PythonRule::PLR0206,
        RuleText::new(
            "Propiedad que recibe parámetros",
            "pasala a un método común",
            "Una propiedad se lee sin llamarla, así que sus parámetros de más nunca reciben un valor.",
            "@property def size(self, scale) pasa a def size(self, scale).",
        ),
    ),
    (
        PythonRule::UP019,
        RuleText::new(
            "typing.Text en vez de str",
            "usá str",
            "typing.Text es un alias de str que quedó por Python 2 y está obsoleto.",
            "name: Text pasa a name: str.",
        ),
    ),
    (
        PythonRule::PIE796,
        RuleText::new(
            "Valor repetido en un Enum",
            "dale a cada miembro su propio valor",
            "Un valor repetido convierte al miembro en un alias del primero en vez de un miembro nuevo.",
            "BLUE = 1 después de RED = 1 pasa a BLUE = 3.",
        ),
    ),
];
