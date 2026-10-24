mod checks;
mod classic;
mod formats;
mod idioms;
mod modern;
mod small_checks;
mod typing;

use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;
use checks::CHECK_TEXTS;
use classic::CLASSIC_TEXTS;
use formats::FORMAT_TEXTS;
use idioms::IDIOM_TEXTS;
use modern::MODERN_TEXTS;
use small_checks::SMALL_CHECK_TEXTS;
use typing::TYPING_TEXTS;

const UNKNOWN: RuleText = RuleText::new(
    "Regla de ruff",
    "seguí la indicación de la regla",
    "ruff la marca en su conjunto por defecto.",
    "Ver la documentación de ruff.",
);

pub fn python(rule: PythonRule) -> RuleText {
    CLASSIC_TEXTS
        .iter()
        .chain(MODERN_TEXTS.iter())
        .chain(IDIOM_TEXTS.iter())
        .chain(CHECK_TEXTS.iter())
        .chain(SMALL_CHECK_TEXTS.iter())
        .chain(TYPING_TEXTS.iter())
        .chain(FORMAT_TEXTS.iter())
        .find(|(candidate, _)| *candidate == rule)
        .map_or(UNKNOWN, |(_, text)| *text)
}
