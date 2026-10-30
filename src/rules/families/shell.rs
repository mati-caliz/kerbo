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
    Sc3001 => "sc3001",
    Sc3002 => "sc3002",
    Sc3003 => "sc3003",
    Sc3004 => "sc3004",
    Sc3005 => "sc3005",
    Sc3006 => "sc3006",
    Sc3007 => "sc3007",
    Sc3008 => "sc3008",
    Sc3009 => "sc3009",
    Sc3010 => "sc3010",
    Sc3011 => "sc3011",
    Sc3012 => "sc3012",
    Sc3014 => "sc3014",
    Sc3015 => "sc3015",
    Sc3016 => "sc3016",
    Sc3017 => "sc3017",
    Sc3018 => "sc3018",
    Sc3019 => "sc3019",
    Sc3020 => "sc3020",
    Sc3021 => "sc3021",
    Sc3022 => "sc3022",
    Sc3023 => "sc3023",
    Sc3024 => "sc3024",
    Sc3025 => "sc3025",
    Sc3026 => "sc3026",
    Sc3028 => "sc3028",
    Sc3029 => "sc3029",
    Sc3030 => "sc3030",
    Sc3031 => "sc3031",
    Sc3032 => "sc3032",
    Sc3033 => "sc3033",
    Sc3034 => "sc3034",
    Sc3035 => "sc3035",
    Sc3036 => "sc3036",
    Sc3037 => "sc3037",
    Sc3038 => "sc3038",
    Sc3039 => "sc3039",
    Sc3040 => "sc3040",
    Sc3041 => "sc3041",
    Sc3042 => "sc3042",
    Sc3043 => "sc3043",
    Sc3044 => "sc3044",
    Sc3045 => "sc3045",
    Sc3046 => "sc3046",
    Sc3047 => "sc3047",
    Sc3048 => "sc3048",
    Sc3049 => "sc3049",
    Sc3050 => "sc3050",
    Sc3052 => "sc3052",
    Sc3053 => "sc3053",
    Sc3054 => "sc3054",
    Sc3055 => "sc3055",
    Sc3056 => "sc3056",
    Sc3057 => "sc3057",
    Sc3058 => "sc3058",
    Sc3059 => "sc3059",
    Sc3060 => "sc3060",
    Sc3061 => "sc3061",
    Sc3062 => "sc3062",
    Sc3063 => "sc3063",
    Sc3064 => "sc3064",
    Sc3065 => "sc3065",
    Sc3066 => "sc3066",
    Sc3067 => "sc3067",
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
