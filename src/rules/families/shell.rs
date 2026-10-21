use super::{ComposeRule, DockerfileRule, EslintRule, MigrationRule, PythonRule, RuleId};

rule_family!(ShellRule {
    Sc2004 => "sc2004",
    Sc2005 => "sc2005",
    Sc2006 => "sc2006",
    Sc2015 => "sc2015",
    Sc2016 => "sc2016",
    Sc2034 => "sc2034",
    Sc2046 => "sc2046",
    Sc2059 => "sc2059",
    Sc2064 => "sc2064",
    Sc2068 => "sc2068",
    Sc2069 => "sc2069",
    Sc2086 => "sc2086",
    Sc2116 => "sc2116",
    Sc2148 => "sc2148",
    Sc2155 => "sc2155",
    Sc2162 => "sc2162",
    Sc2164 => "sc2164",
    Sc2166 => "sc2166",
    Sc2181 => "sc2181",
    Sc2206 => "sc2206",
    Sc2207 => "sc2207",
    Sc2223 => "sc2223",
});

pub const fn wrap_shell<'slots>(slots: &'slots mut [RuleId], rules: &[ShellRule]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Shell(*rule);
            wrap_shell(free, others)
        }
        (free, _) => free,
    }
}

pub fn shell_index(rule: ShellRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(ComposeRule::ALL.len())
        .saturating_add(PythonRule::ALL.len())
        .saturating_add(EslintRule::ALL.len())
        .saturating_add(rule.position())
}
