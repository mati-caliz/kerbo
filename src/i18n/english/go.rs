use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;

const UNKNOWN: RuleText = RuleText::new(
    "golangci-lint rule",
    "follow the rule's advice",
    "golangci-lint reports it by default.",
    "See the golangci-lint documentation.",
);
const TEXTS: [RuleText; GoRule::ALL.len()] = [RuleText::new(
    "Assignment nobody reads before it is overwritten or the function ends",
    "drop the assignment or use the value before assigning again",
    "The value is computed and thrown away: either the computation is extra or a use is missing, and the latter is usually a lost bug, like an err nobody checks.",
    "x := load(); x = other() becomes x := other().",
)];

pub fn go(rule: GoRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
