use super::{ComposeRule, DockerfileRule, EslintRule, MigrationRule, PythonRule, RuleId, ShellRule};

rule_family!(GoRule {
    Ineffassign => "ineffassign",
});

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
