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
    RuleText::new(
        "Modificador fuera del orden de la JLS, o anotación después de los modificadores",
        "ordená los modificadores como la JLS y poné las anotaciones primero",
        "El orden de la JLS (public, protected, private, abstract, default, static, final...) es el que todos esperan leer; una anotación entre los modificadores se pierde.",
        "final static public int X se vuelve public static final int X.",
    ),
    RuleText::new(
        "Corchetes de arreglo al estilo C, después del nombre",
        "poné los corchetes en el tipo",
        "En Java el arreglo es parte del tipo: String[] args se lee de corrido; String args[] es la forma de C.",
        "String args[] se vuelve String[] args.",
    ),
    RuleText::new(
        "Varias variables en una misma sentencia o en la misma línea",
        "declarala en su propia sentencia y su propia línea",
        "Una variable por declaración se lee y se comenta sola, y no esconde un tipo de arreglo distinto entre comas.",
        "int a, b; se vuelve int a; e int b; en dos líneas.",
    ),
    RuleText::new(
        "Interfaz que sólo tiene constantes",
        "pasá las constantes a una clase final o a un enum",
        "Una interfaz describe un tipo; usarla para juntar constantes las mete en el espacio de nombres de quien la implementa.",
        "interface Limits { int MAX = 10; } se vuelve final class Limits { static final int MAX = 10; }.",
    ),
    RuleText::new(
        "switch sin default",
        "agregá un default, aunque sea para fallar",
        "Sin default, un valor nuevo pasa de largo en silencio. Los switch con patrones o case null quedan afuera: ahí el compilador ya exige cubrir todo.",
        "switch (estado) { case ABIERTO: ... } suma default: throw new IllegalStateException(...).",
    ),
    RuleText::new(
        "if, else, for, while o do sin llaves",
        "poné el cuerpo entre llaves",
        "Sin llaves, la segunda línea que alguien agrega queda afuera del if o del bucle aunque esté indentada.",
        "if (lleno) vaciar(); se vuelve if (lleno) { vaciar(); }.",
    ),
    RuleText::new(
        "Bloque vacío en un if, un bucle, un try, un finally o un switch",
        "completá el bloque o sacalo",
        "Un bloque sin sentencias suele ser código que faltó; un comentario solo no alcanza. El catch queda afuera: eso lo mira otra regla.",
        "if (lleno) { } se borra, o se completa.",
    ),
    RuleText::new(
        "Bloque suelto adentro de otro",
        "sacá el bloque o extraelo a un método",
        "Un bloque suelto sólo achica el alcance de una variable: si hace falta, es un método con nombre.",
        "{ int parcial = calcular(); usar(parcial); } se vuelve usarParcial();.",
    ),
    RuleText::new(
        "Asignación adentro de otra expresión",
        "hacé la asignación en su propia sentencia",
        "Una asignación escondida en una condición o un argumento se lee como una comparación. El idioma while ((linea = leer()) != null) se acepta.",
        "if ((total = sumar()) > 0) se vuelve total = sumar(); if (total > 0).",
    ),
    RuleText::new(
        "Expresión booleana con un true o un false de más",
        "usá la condición directamente",
        "Comparar con true o negar un literal no agrega nada y se lee peor.",
        "if (activo == true) se vuelve if (activo).",
    ),
    RuleText::new(
        "equals sin hashCode, o hashCode sin equals",
        "definí los dos",
        "Dos objetos iguales tienen que dar el mismo hashCode: si se define uno solo, los HashMap y HashSet se rompen en silencio.",
        "Una clase con equals(Object) suma hashCode() con los mismos campos.",
    ),
    RuleText::new(
        "Clase utilitaria (sólo static) con un constructor público o implícito",
        "agregá un constructor privado",
        "Si una clase sólo tiene métodos y campos static, instanciarla no tiene sentido: un constructor privado lo impide.",
        "final class Textos { static String limpiar(...) } suma private Textos() {}.",
    ),
    RuleText::new(
        "Comentario con TODO:",
        "hacelo ahora o pasalo a un issue",
        "Un TODO: en el código no tiene dueño ni fecha; en un issue sí.",
        "// TODO: validar se borra y queda un issue.",
    ),
    RuleText::new(
        "Import de un paquete interno de la JDK (sun.*)",
        "usá la API pública equivalente",
        "Los paquetes sun.* no son API: cambian o desaparecen entre versiones de Java.",
        "import sun.misc.BASE64Encoder; se vuelve import java.util.Base64;.",
    ),
    RuleText::new(
        "Archivo con tabs (se marca el primero)",
        "reemplazá los tabs por espacios",
        "Cada editor muestra un tab con otro ancho, y la indentación se desarma.",
        "Un tab al principio de la línea se vuelve cuatro espacios.",
    ),
    RuleText::new(
        "Línea con espacios al final",
        "borrá los espacios del final",
        "Los espacios al final no se ven y ensucian cada diff que los toca.",
        "return x;␣␣ se vuelve return x;.",
    ),
    RuleText::new(
        "Archivo sin salto de línea al final",
        "terminá el archivo con un salto de línea",
        "Sin el salto final, la última línea no es una línea para muchas herramientas, y cada cambio la muestra modificada.",
        "Se agrega \\n al final.",
    ),
    RuleText::new(
        "Método que se puede sobrescribir sin Javadoc que diga cómo",
        "hacé la clase o el método final, o documentá cómo extenderlo",
        "En una clase que se puede heredar, un método público o protegido con cuerpo es un punto de extensión: sin Javadoc, quien lo sobrescribe no sabe qué tiene que conservar.",
        "public void guardar() { ... } se vuelve public final void guardar() { ... }, o lleva Javadoc.",
    ),
    RuleText::new(
        "Variable o parámetro que tapa un campo",
        "cambiale el nombre",
        "Con el mismo nombre que un campo, un this que falta cambia en silencio qué se lee o se escribe.",
        "void setTotal(int total) { this.total = total; } se vuelve void setTotal(int value) { total = value; }.",
    ),
    RuleText::new(
        "Campo que no es privado",
        "hacelo privado y exponelo con métodos",
        "Un campo visible es parte de la API: cualquiera lo cambia sin pasar por la clase, que ya no puede validar ni cambiar cómo lo guarda. Las constantes static final quedan afuera.",
        "public int total; se vuelve private int total; con int total() { return total; }.",
    ),
    RuleText::new(
        "Declaración fuera del orden: variables static, de instancia, constructores y métodos",
        "movela a su lugar",
        "Con un orden fijo (y las variables de pública a privada) se encuentra cada cosa sin buscarla.",
        "Un campo static después de los de instancia sube al principio de la clase.",
    ),
    RuleText::new(
        "Variable local final con un nombre que no es camelCase",
        "nombrala con minúscula inicial, letras y números",
        "Una variable local, aunque sea final, no es una constante: va en camelCase (^([a-z][a-zA-Z0-9]*|_)$). También los recursos de un try.",
        "final int Total = sumar(); se vuelve final int total = sumar();.",
    ),
];

pub fn java(rule: JavaRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
