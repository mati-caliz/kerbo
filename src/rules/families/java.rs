use super::{ComposeRule, DockerfileRule, EslintRule, GoRule, MigrationRule, PythonRule, RuleId, ShellRule};

rule_family!(JavaRule {
    TypeName => "type-name",
    MethodName => "method-name",
    ConstantName => "constant-name",
    StaticVariableName => "static-variable-name",
    MemberName => "member-name",
    ParameterName => "parameter-name",
    UpperEll => "upper-ell",
    EmptyStatement => "empty-statement",
});

pub const fn wrap_java<'slots>(slots: &'slots mut [RuleId], rules: &[JavaRule]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Java(*rule);
            wrap_java(free, others)
        }
        (free, _) => free,
    }
}

pub fn java_index(rule: JavaRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(ComposeRule::ALL.len())
        .saturating_add(PythonRule::ALL.len())
        .saturating_add(EslintRule::ALL.len())
        .saturating_add(ShellRule::ALL.len())
        .saturating_add(GoRule::ALL.len())
        .saturating_add(rule.position())
}
