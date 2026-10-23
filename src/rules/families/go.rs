use super::{ComposeRule, DockerfileRule, EslintRule, MigrationRule, PythonRule, RuleId, ShellRule};

rule_family!(GoRule {
    Ineffassign => "ineffassign",
    St1005 => "st1005",
});

const STATICCHECK: &str = "staticcheck";

impl GoRule {
    // El linter de golangci-lint que la reporta: es el nombre que silencia un `//nolint`.
    pub const fn linter(self) -> &'static str {
        match self {
            Self::Ineffassign => self.as_str(),
            Self::St1005 => STATICCHECK,
        }
    }
}

pub const fn wrap_go<'slots>(slots: &'slots mut [RuleId], rules: &[GoRule]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Go(*rule);
            wrap_go(free, others)
        }
        (free, _) => free,
    }
}

pub fn go_index(rule: GoRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(ComposeRule::ALL.len())
        .saturating_add(PythonRule::ALL.len())
        .saturating_add(EslintRule::ALL.len())
        .saturating_add(ShellRule::ALL.len())
        .saturating_add(rule.position())
}
