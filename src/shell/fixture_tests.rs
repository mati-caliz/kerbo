use super::tests::found;

const BASHISM_PREFIX: &str = "sc30";

// Las reglas de shell de antes de las de `Checks/Commands.hs` y `Analytics.hs`, que tienen sus
// propios fixtures.
const EARLIER_RULES: &[&str] = &[
    "sc2004", "sc2005", "sc2006", "sc2015", "sc2016", "sc2034", "sc2046", "sc2059", "sc2064", "sc2068",
    "sc2069", "sc2086", "sc2116", "sc2148", "sc2155", "sc2162", "sc2164", "sc2166", "sc2181", "sc2206",
    "sc2207", "sc2223",
];

macro_rules! fixture {
    ($name:literal) => {
        (
            include_str!(concat!("fixtures/", $name, ".sh")),
            include_str!(concat!("fixtures/", $name, ".shellcheck")),
        )
    };
}

const FIXTURES: &[(&str, &str)] = &[
    fixture!("commands"),
    fixture!("commands_sh"),
    fixture!("structure"),
    fixture!("structure_ksh"),
    fixture!("structure_sh"),
    fixture!("comparisons"),
    fixture!("control"),
    fixture!("command_shapes"),
    fixture!("equals"),
    fixture!("equals_sh"),
    fixture!("equals_dash"),
    fixture!("equals_ksh"),
    fixture!("command_details"),
    fixture!("command_details_sh"),
    fixture!("reachability"),
];

#[test]
fn checks_commands_words_and_sources_like_shellcheck() {
    for (script, shellcheck) in FIXTURES {
        let expected: Vec<String> = shellcheck.lines().map(str::to_owned).collect();
        let mut actual: Vec<String> = found(script)
            .into_iter()
            .filter(|(code, _, _)| {
                !code.starts_with(BASHISM_PREFIX) && !EARLIER_RULES.contains(&code.as_str())
            })
            .map(|(code, line, column)| format!("{line}:{column} {code}"))
            .collect();
        actual.sort_by_key(|entry| {
            let (position, code) = entry.split_once(' ').unwrap_or_default();
            let (line, column) = position.split_once(':').unwrap_or_default();
            (
                line.parse::<usize>().unwrap_or_default(),
                column.parse::<usize>().unwrap_or_default(),
                code.to_owned(),
            )
        });
        assert_eq!(actual, expected);
    }
}
