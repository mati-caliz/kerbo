use crate::i18n::catalog::RuleText;
use crate::lint::JavadocTagProblem;
use crate::rules::JavaRule;

pub fn tag_problem(problem: JavadocTagProblem, subject: &str) -> String {
    match problem {
        JavadocTagProblem::Missing => format!("Al Javadoc le falta `{subject}`"),
        JavadocTagProblem::Unused => format!("`{subject}` no corresponde a nada de la declaración"),
        JavadocTagProblem::Duplicate => format!("`{subject}` está repetida en el Javadoc"),
        JavadocTagProblem::Unknown => {
            format!("`{subject}` no es una etiqueta de bloque estándar de Javadoc")
        }
        JavadocTagProblem::InvalidInheritDoc => {
            format!("`{subject}` no hereda nada en un constructor, un método static o uno privado")
        }
    }
}

pub const RULE_COUNT: usize = 6;

const RULE_TEXTS: [(JavaRule, RuleText); RULE_COUNT] = [
    (
        JavaRule::JavadocVariable,
        RuleText::new(
            "Campo o constante de enum sin Javadoc",
            "escribile un Javadoc que diga qué guarda",
            "sun_checks pide Javadoc en todos los campos, también los privados: qué significa el valor, en qué unidad está y quién lo cambia no se deduce del nombre. serialVersionUID queda afuera. Sólo lo pide el preset strict.",
            "private int reintentos; suma /** Cuántas veces se reintenta antes de fallar. */ arriba.",
        ),
    ),
    (
        JavaRule::MissingJavadocMethod,
        RuleText::new(
            "Método o constructor público sin Javadoc",
            "escribile un Javadoc con lo que hace, sus parámetros y lo que devuelve",
            "Lo público de un tipo público es la API: quien la usa lee el Javadoc, no el cuerpo. Los que llevan @Override heredan el del método que pisan. Sólo lo pide el preset strict.",
            "public Factura emitir(Pedido pedido) suma /** Emite la factura del pedido. @param pedido ... @return ... */.",
        ),
    ),
    (
        JavaRule::JavadocMethod,
        RuleText::new(
            "Javadoc de método que no coincide con la firma",
            "agregá la etiqueta que falta o sacá la que sobra",
            "Un Javadoc que existe tiene que documentar cada parámetro, cada parámetro de tipo y lo que devuelve, sin etiquetas de parámetros que ya no están: si no, miente. Con {@inheritDoc} o @Override no se piden las que faltan. Sólo lo pide el preset strict.",
            "Un /** Suma. */ arriba de int sumar(int a, int b) suma @param a, @param b y @return.",
        ),
    ),
    (
        JavaRule::JavadocType,
        RuleText::new(
            "Javadoc de tipo con una etiqueta desconocida o sin @param de un parámetro de tipo",
            "documentá cada <T> y cada componente del record, y usá etiquetas estándar",
            "Como JavadocType de sun_checks: cada parámetro de tipo (y cada componente de un record) lleva su @param, y una etiqueta de bloque que Checkstyle no conoce, como @apiNote, se marca. Sólo lo pide el preset strict.",
            "/** Caché. */ class Cache<K, V> suma @param <K> y @param <V>.",
        ),
    ),
    (
        JavaRule::JavadocPackage,
        RuleText::new(
            "Paquete sin package-info.java",
            "agregá un package-info.java con el Javadoc del paquete",
            "El Javadoc del paquete dice para qué está y qué entra ahí; sin package-info.java no hay dónde escribirlo. Se avisa una vez por carpeta, en el primer archivo por orden alfabético. Sólo lo pide el preset strict.",
            "src/main/java/ar/pagos suma package-info.java con /** Cobros y devoluciones. */ package ar.pagos;.",
        ),
    ),
    (
        JavaRule::InvalidJavadocPosition,
        RuleText::new(
            "Javadoc en un lugar donde no documenta nada",
            "movelo arriba de la declaración o pasalo a comentario común",
            "Un /** */ adentro de un método, entre una anotación y la declaración, o seguido de otro Javadoc no aparece en ninguna documentación: o está mal puesto o es un comentario común. Sólo lo pide el preset strict.",
            "@Override /** Cierra. */ public void close() se vuelve /** Cierra. */ @Override public void close().",
        ),
    ),
];

pub fn rule_text(rule: JavaRule) -> Option<RuleText> {
    RULE_TEXTS
        .iter()
        .find(|(known, _)| *known == rule)
        .map(|(_, text)| *text)
}
