use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;

const UNKNOWN: RuleText = RuleText::new(
    "Regla de golangci-lint",
    "seguí la indicación de la regla",
    "golangci-lint la marca por defecto.",
    "Ver la documentación de golangci-lint.",
);
const TEXTS: [RuleText; GoRule::ALL.len()] = [
    RuleText::new(
        "Asignación que nadie lee antes de que se pise o termine la función",
        "borrá la asignación o usá el valor antes de volver a asignar",
        "El valor se calcula y se tira: o sobra el cálculo o falta usarlo, y lo segundo suele ser un error que se perdió, como un err que nadie revisa.",
        "x := cargar(); x = otro() se vuelve x := otro().",
    ),
    RuleText::new(
        "Mensaje de error con mayúscula al principio o puntuación al final (ST1005 de staticcheck)",
        "empezá el mensaje en minúscula y sacale el punto, los dos puntos, el signo o el salto de línea del final",
        "Los errores se encadenan: fmt.Errorf(\"guardar: %w\", err) arma una sola oración, y una mayúscula o un punto en el medio la cortan.",
        "errors.New(\"No se encontró el pedido.\") se vuelve errors.New(\"no se encontró el pedido\").",
    ),
];

pub fn go(rule: GoRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
