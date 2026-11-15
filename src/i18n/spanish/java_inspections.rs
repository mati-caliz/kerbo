use crate::i18n::catalog::RuleText;
use crate::rules::JavaRule;

pub const RULE_COUNT: usize = 16;

const RULE_TEXTS: [(JavaRule, RuleText); RULE_COUNT] = [
    (
        JavaRule::UnnecessaryUnicodeEscape,
        RuleText::new(
            "Escape Unicode de un carácter que se puede escribir tal cual",
            "escribí el carácter en vez del escape",
            "Un \\u0041 obliga a buscar qué letra es; el archivo está en UTF-8 y el carácter se lee directo. Como UnnecessaryUnicodeEscape de IntelliJ: quedan los controles, los espacios raros, los invisibles, los sin asignar y las marcas combinantes, que no se ven bien escritos.",
            "String nombre = \"Jos\\u00e9\"; se vuelve String nombre = \"José\";.",
        ),
    ),
    (
        JavaRule::Java8ListSort,
        RuleText::new(
            "Collections.sort(lista, comparador) donde alcanza con lista.sort(comparador)",
            "llamá a sort sobre la lista",
            "Desde Java 8 la lista se ordena sola con List.sort, que algunas implementaciones hacen más rápido que copiar a un arreglo. Como Java8ListSort de IntelliJ: sólo con release 8 o más.",
            "Collections.sort(pedidos, porFecha); se vuelve pedidos.sort(porFecha);.",
        ),
    ),
    (
        JavaRule::SizeReplaceableByIsEmpty,
        RuleText::new(
            "size() o length() comparado con 0 o 1 donde alcanza con isEmpty()",
            "usá isEmpty(), negado si la comparación pregunta si hay algo",
            "isEmpty() dice lo que se pregunta sin hacer la cuenta, y en algunas colecciones es más barato que contar. Como SizeReplaceableByIsEmpty de IntelliJ: sólo cuando el tipo declarado tiene isEmpty() (String, las colecciones y mapas de Java y de guava, CharSequence desde Java 15).",
            "if (pedidos.size() == 0) se vuelve if (pedidos.isEmpty()), y nombre.length() > 0, !nombre.isEmpty().",
        ),
    ),
    (
        JavaRule::ManualMinMaxCalculation,
        RuleText::new(
            "Mínimo o máximo calculado a mano con un ternario o un if",
            "usá Math.min o Math.max",
            "Math.min(a, b) dice qué se calcula; el ternario o el if obligan a leer la comparación y las dos ramas para darse cuenta, y es fácil dar vuelta el signo. Como ManualMinMaxCalculation de IntelliJ: sólo con los dos lados del mismo tipo numérico (int, long, float o double).",
            "int tope = (pedidos < limite) ? pedidos : limite; se vuelve int tope = Math.min(pedidos, limite);.",
        ),
    ),
    (
        JavaRule::DefaultAnnotationParam,
        RuleText::new(
            "Elemento de una anotación escrito con el valor que ya tiene por defecto",
            "sacá el elemento",
            "Repetir el valor por defecto es ruido y obliga a fijarse si es distinto. Como DefaultAnnotationParam de IntelliJ: sólo con las anotaciones que declara el proyecto, que es donde se ve el valor por defecto.",
            "@Reintento(intentos = 3) con intentos() default 3 se vuelve @Reintento.",
        ),
    ),
    (
        JavaRule::Java8MapApi,
        RuleText::new(
            "Valor leído con get, calculado si es null y guardado con put",
            "usá Map.computeIfAbsent",
            "computeIfAbsent dice en una llamada lo que el get, el chequeo de null y el put reparten en varias líneas, y busca la clave una sola vez. Como Java8MapApi de IntelliJ en la forma de computeIfAbsent: sólo cuando el tipo declarado es un Map, el release es 8 o más y lo calculado no tiene efectos (un arreglo o un literal).",
            "Class<?>[] resultado = cache.get(tipo); if (resultado == null) { resultado = new Class[] { tipo }; cache.put(tipo, resultado); } se vuelve Class<?>[] resultado = cache.computeIfAbsent(tipo, clave -> new Class[] { clave });.",
        ),
    ),
    (
        JavaRule::FinalStaticMethod,
        RuleText::new(
            "Método static declarado final",
            "sacá el final",
            "Un método static no se redefine: final sólo impide que una subclase declare otro con la misma firma, que casi nunca es lo que se quiso, y suele delatar la idea equivocada de que evita un override. Como FinalStaticMethod de IntelliJ, que Checkstyle y PMD no tienen.",
            "public static final int doble(int valor) se vuelve public static int doble(int valor).",
        ),
    ),
    (
        JavaRule::TransientStaticField,
        RuleText::new(
            "Campo static declarado transient",
            "sacá el transient",
            "La serialización nunca guarda los campos static, así que transient no cambia nada y hace pensar que sí. Como UnnecessaryModifier de IntelliJ, que Checkstyle y PMD no tienen.",
            "private static transient Logger registro; se vuelve private static Logger registro;.",
        ),
    ),
    (
        JavaRule::TextBlockMigration,
        RuleText::new(
            "Literales de texto concatenados con dos o más saltos de línea, donde un bloque de texto se lee mejor",
            "escribilo como bloque de texto",
            "Un bloque de texto muestra el texto como va a salir, sin los escapes de salto de línea ni el + al final de cada renglón. Como TextBlockMigration de IntelliJ: sólo cuando todas las partes son literales, hay al menos dos saltos de línea escritos como escape y ningún comentario en el medio, con release 15 o más.",
            "Dos literales seguidos que terminan en salto de línea, como la consulta select y su from, se vuelven un bloque de texto con las dos líneas.",
        ),
    ),
    (
        JavaRule::SequencedCollectionMethod,
        RuleText::new(
            "get(0), get(size() - 1), remove(0), add(0, x) o iterator().next() donde Java 21 tiene getFirst, getLast, removeFirst o addFirst",
            "llamá al método de la colección secuenciada",
            "getFirst() dice qué elemento se busca; get(0) y la cuenta de size() - 1 obligan a deducirlo. Como SequencedCollectionMethodCanBeUsed de IntelliJ: con release 21 o más, cuando el tipo declarado es una lista u otra colección secuenciada, y no en una serie como get(0), get(1).",
            "pedidos.get(pedidos.size() - 1) se vuelve pedidos.getLast().",
        ),
    ),
    (
        JavaRule::MathClampMigration,
        RuleText::new(
            "Math.min y Math.max anidados donde Java 21 tiene Math.clamp",
            "usá Math.clamp",
            "Math.clamp(valor, minimo, maximo) dice que el valor queda dentro de un rango; el min y el max anidados obligan a deducir cuál es cada borde. Como MathClampMigration de IntelliJ: con release 21 o más, cuando algo dentro de la llamada interna no es una constante.",
            "Math.max(0, Math.min(60, minutos)) se vuelve Math.clamp(minutos, 0, 60).",
        ),
    ),
    (
        JavaRule::StringRepeatCanBeUsed,
        RuleText::new(
            "append(texto.repeat(n)) donde Java 21 tiene StringBuilder.repeat",
            "llamá a repeat sobre el builder",
            "builder.repeat(texto, n) escribe las copias directo en el builder en vez de armar antes un String intermedio. Como StringRepeatCanBeUsed de IntelliJ en su forma de Java 21: el receptor es un StringBuilder o StringBuffer declarado y lo que se repite es un String.",
            "linea.append(\"-\".repeat(ancho)) se vuelve linea.repeat(\"-\", ancho).",
        ),
    ),
    (
        JavaRule::PatternVariableCanBeUsed,
        RuleText::new(
            "Variable local que se inicializa con un cast que un instanceof anterior ya garantiza",
            "declarala en el instanceof como variable de patrón",
            "Con los patrones de Java 16, valor instanceof Pedido pedido chequea y declara de una vez; el cast aparte repite el tipo y puede quedar desfasado del chequeo. Como PatternVariableCanBeUsed de IntelliJ: el instanceof es la condición del if o el bucle que encierra la declaración, o un if anterior del mismo bloque que sale cuando no se cumple, y lo que se castea no se reasigna en el medio.",
            "if (valor instanceof Pedido) { Pedido pedido = (Pedido) valor; ... } se vuelve if (valor instanceof Pedido pedido) { ... }.",
        ),
    ),
    (
        JavaRule::ReadWriteString,
        RuleText::new(
            "Bytes leídos o escritos con Files sólo para pasarlos a String, donde Files.readString y Files.writeString lo hacen",
            "usá Files.readString o Files.writeString",
            "Files.readString(ruta, juego) dice en una llamada lo que new String(Files.readAllBytes(ruta), juego) arma en dos. Como ReadWriteStringCanBeUsed de IntelliJ: readString desde Java 11 y writeString desde Java 12, cuando el juego de caracteres es un Charset.",
            "new String(Files.readAllBytes(ruta), UTF_8) se vuelve Files.readString(ruta, UTF_8).",
        ),
    ),
    (
        JavaRule::Java9CollectionFactory,
        RuleText::new(
            "Copia inmodificable o lista fija armada a mano donde List.of o List.copyOf la hacen",
            "usá List.of, Set.of, Map.of o sus copyOf",
            "List.copyOf(otra) dice en una llamada lo que Collections.unmodifiableList(new ArrayList<>(otra)) arma en dos, y el resultado no acepta nulls. Como Java9CollectionFactory de IntelliJ: copyOf desde Java 10 cuando lo copiado se declara como colección; of desde Java 9 cuando ningún elemento puede ser null.",
            "Collections.unmodifiableList(new ArrayList<>(pedidos)) se vuelve List.copyOf(pedidos).",
        ),
    ),
    (
        JavaRule::EnhancedSwitchMigration,
        RuleText::new(
            "Switch viejo que devuelve en todas sus ramas, donde una expresión switch lo dice de una vez",
            "devolvé una expresión switch con case ... ->",
            "return switch (tipo) { case A -> 1; default -> 0; }; muestra que todas las ramas dan un valor y el compilador controla que no falte ninguna. Como EnhancedSwitchMigration de IntelliJ en su forma por defecto: cada rama, de hasta dos sentencias, termina en return o throw, y sin default al switch le sigue un return o un throw.",
            "switch (tipo) { case A: return 1; default: return 0; } se vuelve return switch (tipo) { case A -> 1; default -> 0; };.",
        ),
    ),
];

pub fn rule_text(rule: JavaRule) -> Option<RuleText> {
    RULE_TEXTS
        .iter()
        .find(|(known, _)| *known == rule)
        .map(|(_, text)| *text)
}
