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
    S1034 => "s1034",
    S1035 => "s1035",
    S1009 => "s1009",
    S1011 => "s1011",
    S1017 => "s1017",
    S1029 => "s1029",
    S1030 => "s1030",
    S1032 => "s1032",
    S1038 => "s1038",
    S1039 => "s1039",
    S1040 => "s1040",
    Sa1006 => "sa1006",
    Sa2001 => "sa2001",
    Sa4004 => "sa4004",
    Sa4011 => "sa4011",
    Sa4016 => "sa4016",
    Sa6002 => "sa6002",
    Sa6003 => "sa6003",
    Sa6005 => "sa6005",
    Sa9009 => "sa9009",
    St1008 => "st1008",
    Qf1007 => "qf1007",
    S1004 => "s1004",
    S1006 => "s1006",
    S1010 => "s1010",
    S1012 => "s1012",
    S1023 => "s1023",
    S1024 => "s1024",
    S1028 => "s1028",
    S1033 => "s1033",
    S1037 => "s1037",
    Sa1012 => "sa1012",
    Sa1013 => "sa1013",
    Sa2000 => "sa2000",
    Sa3001 => "sa3001",
    Sa4001 => "sa4001",
    Sa4013 => "sa4013",
    Sa4021 => "sa4021",
    Sa4022 => "sa4022",
    Sa4024 => "sa4024",
    Sa4028 => "sa4028",
    Sa5002 => "sa5002",
    Sa6006 => "sa6006",
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
            | Self::S1002
            | Self::S1034
            | Self::S1035
            | Self::S1009
            | Self::S1011
            | Self::S1017
            | Self::S1029
            | Self::S1030
            | Self::S1032
            | Self::S1038
            | Self::S1039
            | Self::S1040
            | Self::Sa1006
            | Self::Sa2001
            | Self::Sa4004
            | Self::Sa4011
            | Self::Sa4016
            | Self::Sa6002
            | Self::Sa6003
            | Self::Sa6005
            | Self::Sa9009
            | Self::St1008
            | Self::Qf1007
            | Self::S1004
            | Self::S1006
            | Self::S1010
            | Self::S1012
            | Self::S1023
            | Self::S1024
            | Self::S1028
            | Self::S1033
            | Self::S1037
            | Self::Sa1012
            | Self::Sa1013
            | Self::Sa2000
            | Self::Sa3001
            | Self::Sa4001
            | Self::Sa4013
            | Self::Sa4021
            | Self::Sa4022
            | Self::Sa4024
            | Self::Sa4028
            | Self::Sa5002
            | Self::Sa6006 => STATICCHECK,
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
