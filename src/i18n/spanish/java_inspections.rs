use crate::i18n::catalog::RuleText;
use crate::rules::JavaRule;

pub const RULE_COUNT: usize = 8;

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
];

pub fn rule_text(rule: JavaRule) -> Option<RuleText> {
    RULE_TEXTS
        .iter()
        .find(|(known, _)| *known == rule)
        .map(|(_, text)| *text)
}
