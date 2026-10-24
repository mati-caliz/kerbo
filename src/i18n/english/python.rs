mod assorted_checks;
mod checks;
mod classic;
mod formats;
mod idioms;
mod mixed_checks;
mod modern;
mod pylint_errors;
mod small_checks;
mod stub_rules;
mod tail_checks;
mod typing;
mod upgrade_rules;

use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;
use assorted_checks::ASSORTED_CHECK_TEXTS;
use checks::CHECK_TEXTS;
use classic::CLASSIC_TEXTS;
use formats::FORMAT_TEXTS;
use idioms::IDIOM_TEXTS;
use mixed_checks::MIXED_CHECK_TEXTS;
use modern::MODERN_TEXTS;
use pylint_errors::PYLINT_ERROR_TEXTS;
use small_checks::SMALL_CHECK_TEXTS;
use stub_rules::STUB_RULE_TEXTS;
use tail_checks::TAIL_CHECK_TEXTS;
use typing::TYPING_TEXTS;
use upgrade_rules::UPGRADE_RULE_TEXTS;

const UNKNOWN: RuleText = RuleText::new(
    "A ruff rule",
    "follow the rule",
    "ruff reports it in its default set.",
    "See the ruff documentation.",
);

pub fn python(rule: PythonRule) -> RuleText {
    CLASSIC_TEXTS
        .iter()
        .chain(MODERN_TEXTS.iter())
        .chain(IDIOM_TEXTS.iter())
        .chain(CHECK_TEXTS.iter())
        .chain(UPGRADE_RULE_TEXTS.iter())
        .chain(STUB_RULE_TEXTS.iter())
        .chain(PYLINT_ERROR_TEXTS.iter())
        .chain(ASSORTED_CHECK_TEXTS.iter())
        .chain(TAIL_CHECK_TEXTS.iter())
        .chain(MIXED_CHECK_TEXTS.iter())
        .chain(SMALL_CHECK_TEXTS.iter())
        .chain(TYPING_TEXTS.iter())
        .chain(FORMAT_TEXTS.iter())
        .find(|(candidate, _)| *candidate == rule)
        .map_or(UNKNOWN, |(_, text)| *text)
}
