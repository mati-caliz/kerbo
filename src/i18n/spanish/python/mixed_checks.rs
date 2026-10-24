use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: sentencias, llamadas y pytest.
pub const MIXED_CHECK_TEXTS: [(PythonRule, RuleText); 24] = [
    (
        PythonRule::PLC0206,
        RuleText::new(
            "Recorrido de un dict que busca el valor por índice",
            "recorré dict.items()",
            "Buscar cada clave de nuevo adentro del loop es más lento y ruidoso que traer la clave y el valor juntos.",
            "for k in d: print(d[k]) pasa a for k, v in d.items(): print(v).",
        ),
    ),
    (
        PythonRule::PERF402,
        RuleText::new(
            "Loop que copia una lista elemento por elemento",
            "usá list(items) o result.extend(items)",
            "Agregar cada elemento de a uno es más lento que copiar el iterable en una sola llamada.",
            "for x in items: result.append(x) pasa a result = list(items).",
        ),
    ),
    (
        PythonRule::PT031,
        RuleText::new(
            "pytest.warns con varias sentencias",
            "dejá adentro del with sólo la sentencia que tiene que avisar",
            "Si otra sentencia avisa, el test pasa aunque el código que se prueba no lo haga.",
            "with pytest.warns(W): setup(); act() pasa a setup() y después with pytest.warns(W): act().",
        ),
    ),
    (
        PythonRule::PLW0602,
        RuleText::new(
            "global que nunca se asigna",
            "sacá la declaración global",
            "global sólo hace falta para asignar la variable del módulo; para leerla no se necesita.",
            "global DEBUG seguido sólo de lecturas de DEBUG se borra.",
        ),
    ),
    (
        PythonRule::PT014,
        RuleText::new(
            "Caso repetido en pytest.mark.parametrize",
            "sacá el caso repetido",
            "El caso repetido corre el mismo test dos veces sin chequear nada nuevo.",
            "parametrize(\"x\", [1, 2, 1]) pasa a parametrize(\"x\", [1, 2]).",
        ),
    ),
    (
        PythonRule::RUF019,
        RuleText::new(
            "Chequeo de la clave antes de indexar un dict",
            "usá dict.get",
            "Chequear la clave y después indexar busca la clave dos veces; get lo hace una.",
            "\"k\" in d and d[\"k\"] pasa a d.get(\"k\").",
        ),
    ),
    (
        PythonRule::FURB192,
        RuleText::new(
            "sorted() sólo para sacar el mínimo o el máximo",
            "usá min() o max()",
            "Ordenar toda la secuencia para quedarse con un elemento lleva O(n log n) en vez de O(n).",
            "sorted(values)[0] pasa a min(values).",
        ),
    ),
    (
        PythonRule::PLR1716,
        RuleText::new(
            "Comparaciones que se pueden encadenar",
            "encadenalas: a < b < c",
            "Python encadena comparaciones, que se lee mejor y evalúa el valor del medio una sola vez.",
            "a < b and b < c pasa a a < b < c.",
        ),
    ),
    (
        PythonRule::FURB105,
        RuleText::new(
            "Argumento de print() innecesario",
            "sacá el string vacío o el sep",
            "print() ya imprime una línea vacía, y sep sólo importa entre dos o más valores.",
            "print(\"\") pasa a print().",
        ),
    ),
    (
        PythonRule::RUF057,
        RuleText::new(
            "round() que no cambia nada",
            "sacá la llamada a round()",
            "Redondear un entero sin decimales negativos devuelve el mismo entero.",
            "round(1, 0) pasa a 1.",
        ),
    ),
    (
        PythonRule::PLE0605,
        RuleText::new(
            "__all__ que no es una lista ni una tupla",
            "escribí __all__ como lista o tupla de nombres",
            "Las herramientas y from modulo import * leen __all__ como una secuencia de nombres; un string se toma letra por letra.",
            "__all__ = \"Foo\" pasa a __all__ = (\"Foo\",).",
        ),
    ),
    (
        PythonRule::PLW0131,
        RuleText::new(
            "Expresión con nombre usada como sentencia",
            "usá una asignación común",
            "El walrus sólo tiene sentido adentro de otra expresión; suelto es una asignación escrita más difícil.",
            "(a := 42) pasa a a = 42.",
        ),
    ),
    (
        PythonRule::RUF041,
        RuleText::new(
            "Literal anidado",
            "aplanalo en un solo Literal",
            "Un Literal adentro de otro no suma nada: sus valores ya son parte del de afuera.",
            "Literal[Literal[\"a\"], \"b\"] pasa a Literal[\"a\", \"b\"].",
        ),
    ),
    (
        PythonRule::D419,
        RuleText::new(
            "Docstring vacío",
            "escribí el docstring o sacalo",
            "Un docstring vacío promete una documentación que no está.",
            "\"\"\"\"\"\" pasa a \"\"\"Devuelve el promedio de los valores.\"\"\".",
        ),
    ),
    (
        PythonRule::FURB166,
        RuleText::new(
            "int() de un string con el prefijo recortado",
            "usá int(text, 0)",
            "Con base 0, int() lee solo el prefijo 0b, 0o o 0x, sin recortar el string.",
            "int(text[2:], 16) pasa a int(text, 0).",
        ),
    ),
    (
        PythonRule::RUF051,
        RuleText::new(
            "if key in dict antes de un del",
            "usá dict.pop(key, None)",
            "pop con un default borra la clave en un solo paso sin un chequeo aparte.",
            "if k in d: del d[k] pasa a d.pop(k, None).",
        ),
    ),
    (
        PythonRule::UP014,
        RuleText::new(
            "NamedTuple con la sintaxis funcional",
            "escribilo como clase",
            "La sintaxis de clase deja que cada campo tenga su anotación, su default y su docstring.",
            "Point = NamedTuple(\"Point\", [(\"x\", int)]) pasa a class Point(NamedTuple): x: int.",
        ),
    ),
    (
        PythonRule::G201,
        RuleText::new(
            "logging.error con exc_info=True",
            "usá logging.exception",
            "logging.exception registra con nivel error y adjunta el traceback, con menos que escribir.",
            "logging.error(\"failed\", exc_info=True) pasa a logging.exception(\"failed\").",
        ),
    ),
    (
        PythonRule::B035,
        RuleText::new(
            "Comprensión de dict con una clave fija",
            "usá una clave que dependa del loop",
            "Una clave que no cambia se queda sólo con el último valor, así que la comprensión arma un dict de un elemento.",
            "{\"key\": v for v in data} pasa a {v: v for v in data}.",
        ),
    ),
    (
        PythonRule::B005,
        RuleText::new(
            "strip() con caracteres repetidos",
            "usá removeprefix o removesuffix",
            "strip saca cualquiera de los caracteres dados, no el string entero, así que .txt también se come letras del nombre.",
            "name.rstrip(\".txt\") pasa a name.removesuffix(\".txt\").",
        ),
    ),
    (
        PythonRule::PLW0245,
        RuleText::new(
            "super usado sin llamarlo",
            "escribí super()",
            "super sin paréntesis es la clase del builtin, no el padre de este objeto, así que buscar el método falla.",
            "super.__init__() pasa a super().__init__().",
        ),
    ),
    (
        PythonRule::B021,
        RuleText::new(
            "f-string usado como docstring",
            "escribí un string común",
            "Python no toma un f-string como docstring, así que el objeto queda sin documentación.",
            "f\"\"\"Docs {x}\"\"\" pasa a \"\"\"Docs.\"\"\".",
        ),
    ),
    (
        PythonRule::B012,
        RuleText::new(
            "return, break o continue adentro de un finally",
            "sacalo del finally",
            "Salir de un finally así descarta en silencio la excepción que se estaba lanzando.",
            "finally: return 1 pasa a un return después del try.",
        ),
    ),
    (
        PythonRule::B013,
        RuleText::new(
            "Tupla de un solo elemento en un except",
            "atrapá la excepción sin la tupla",
            "Una tupla de una sola excepción es lo mismo que la excepción sola.",
            "except (ValueError,): pasa a except ValueError:.",
        ),
    ),
];
