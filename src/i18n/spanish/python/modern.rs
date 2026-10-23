use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Las reglas del conjunto por defecto de ruff 0.17 fuera del clásico (pyupgrade, flake8-comprehensions
// y compañía).
pub const MODERN_TEXTS: [(PythonRule, RuleText); 10] = [
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
];
