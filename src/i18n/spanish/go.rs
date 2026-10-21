use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;

const UNKNOWN: RuleText = RuleText::new(
    "Regla de golangci-lint",
    "seguí la indicación de la regla",
    "golangci-lint la marca por defecto.",
    "Ver la documentación de golangci-lint.",
);
const TEXTS: [RuleText; GoRule::ALL.len()] = [RuleText::new(
    "Asignación que nadie lee antes de que se pise o termine la función",
    "borrá la asignación o usá el valor antes de volver a asignar",
    "El valor se calcula y se tira: o sobra el cálculo o falta usarlo, y lo segundo suele ser un error que se perdió, como un err que nadie revisa.",
    "x := cargar(); x = otro() se vuelve x := otro().",
)];

pub fn go(rule: GoRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
