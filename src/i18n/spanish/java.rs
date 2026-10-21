use crate::i18n::catalog::RuleText;
use crate::rules::JavaRule;

const UNKNOWN: RuleText = RuleText::new(
    "Regla de Checkstyle",
    "seguí la indicación de la regla",
    "Checkstyle la marca con sun_checks.xml.",
    "Ver la documentación de Checkstyle.",
);
const TEXTS: [RuleText; JavaRule::ALL.len()] = [
    RuleText::new(
        "Tipo con un nombre que no es PascalCase",
        "nombralo con mayúscula inicial, letras y números",
        "Clases, interfaces, enums, records y anotaciones van en PascalCase (^[A-Z][a-zA-Z0-9]*$): así se distinguen de un vistazo de variables y métodos.",
        "class order_item se vuelve class OrderItem.",
    ),
    RuleText::new(
        "Método con un nombre que no es camelCase, o que se llama igual que su clase",
        "nombralo con minúscula inicial, letras y números",
        "Los métodos van en camelCase (^[a-z][a-zA-Z0-9]*$); uno que se llama igual que la clase se confunde con un constructor. Los que llevan @Override no se miran: el nombre lo decide quien los declara.",
        "void Do_thing() se vuelve void doThing().",
    ),
    RuleText::new(
        "Constante (static final) que no está en MAYÚSCULAS_CON_GUIONES",
        "nombrala en mayúsculas, con guiones bajos entre palabras",
        "Un campo static final, y cualquier campo de una interfaz, es una constante y va en ^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$; serialVersionUID queda afuera.",
        "static final int maxSize = 10; se vuelve static final int MAX_SIZE = 10;.",
    ),
    RuleText::new(
        "Variable static con un nombre que no es camelCase",
        "nombrala con minúscula inicial, letras y números",
        "Un campo static que no es final va en camelCase (^[a-z][a-zA-Z0-9]*$), como cualquier variable: las mayúsculas quedan para las constantes.",
        "static int Counter; se vuelve static int counter;.",
    ),
    RuleText::new(
        "Campo con un nombre que no es camelCase",
        "nombralo con minúscula inicial, letras y números",
        "Los campos de instancia van en camelCase (^[a-z][a-zA-Z0-9]*$), sin guiones bajos ni prefijos como m_.",
        "private int m_count; se vuelve private int count;.",
    ),
    RuleText::new(
        "Parámetro con un nombre que no es camelCase",
        "nombralo con minúscula inicial, letras y números",
        "Los parámetros de métodos y constructores van en camelCase (^[a-z][a-zA-Z0-9]*$).",
        "void put(String Key) se vuelve void put(String key).",
    ),
    RuleText::new(
        "Literal long con una l minúscula",
        "usá L mayúscula",
        "La l minúscula se confunde con un 1: 10l parece 101.",
        "long total = 10l; se vuelve long total = 10L;.",
    ),
    RuleText::new(
        "Sentencia vacía: un ; suelto",
        "borrá el ; o completá el cuerpo",
        "Un ; suelto no hace nada, y después de un for, un while o un if deja el bloque que sigue afuera del bucle o de la condición.",
        "for (int i = 0; i < n; i++); { sumar(i); } se vuelve for (int i = 0; i < n; i++) { sumar(i); }.",
    ),
];

pub fn java(rule: JavaRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
