use super::{ComposeRule, DockerfileRule, EslintRule, MigrationRule, PythonRule, RuleId, ShellRule};

rule_family!(GoRule {
    Ineffassign => "ineffassign",
    Errcheck => "errcheck",
    St1005 => "st1005",
    Qf1003 => "qf1003",
    Qf1004 => "qf1004",
    Qf1008 => "qf1008",
    Qf1012 => "qf1012",
    Sa1019 => "sa1019",
    Qf1001 => "qf1001",
    Stdmethods => "stdmethods",
    St1012 => "st1012",
    Qf1011 => "qf1011",
    St1023 => "st1023",
    Qf1002 => "qf1002",
    S1005 => "s1005",
    Sa9003 => "sa9003",
    Qf1006 => "qf1006",
    St1019 => "st1019",
    Sa1029 => "sa1029",
    Sa9004 => "sa9004",
    Qf1009 => "qf1009",
    St1013 => "st1013",
    S1000 => "s1000",
    St1017 => "st1017",
    St1006 => "st1006",
    S1008 => "s1008",
    S1002 => "s1002",
    Unused => "unused",
});

const STATICCHECK: &str = "staticcheck";
const GOVET: &str = "govet";

impl GoRule {
    // El linter de golangci-lint que la reporta: es el nombre que silencia un `//nolint`.
    pub const fn linter(self) -> &'static str {
        match self {
            Self::Stdmethods => GOVET,
            Self::Ineffassign | Self::Errcheck | Self::Unused => self.as_str(),
            Self::St1005
            | Self::Qf1003
            | Self::Qf1004
            | Self::Qf1008
            | Self::Qf1012
            | Self::Sa1019
            | Self::Qf1001
            | Self::St1012
            | Self::Qf1011
            | Self::St1023
            | Self::Qf1002
            | Self::S1005
            | Self::Sa9003
            | Self::Qf1006
            | Self::St1019
            | Self::Sa1029
            | Self::Sa9004
            | Self::Qf1009
            | Self::St1013
            | Self::S1000
            | Self::St1017
            | Self::St1006
            | Self::S1008
            | Self::S1002 => STATICCHECK,
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
