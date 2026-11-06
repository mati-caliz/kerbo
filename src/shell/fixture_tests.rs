use super::tests::found;

const BASHISM_PREFIX: &str = "sc30";

// Las reglas de shell de antes de las de `Checks/Commands.hs` y `Analytics.hs`, que tienen sus
// propios fixtures.
const EARLIER_RULES: &[&str] = &[
    "sc2004", "sc2005", "sc2006", "sc2015", "sc2016", "sc2034", "sc2046", "sc2059", "sc2064", "sc2068",
    "sc2069", "sc2086", "sc2116", "sc2148", "sc2155", "sc2162", "sc2164", "sc2166", "sc2181", "sc2206",
    "sc2207", "sc2223",
];

#[test]
fn checks_commands_words_and_sources_like_shellcheck() {
    let fixtures = [
        (
            include_str!("fixtures/commands.sh"),
            include_str!("fixtures/commands.shellcheck"),
        ),
        (
            include_str!("fixtures/commands_sh.sh"),
            include_str!("fixtures/commands_sh.shellcheck"),
        ),
        (
            include_str!("fixtures/structure.sh"),
            include_str!("fixtures/structure.shellcheck"),
        ),
        (
            include_str!("fixtures/structure_ksh.sh"),
            include_str!("fixtures/structure_ksh.shellcheck"),
        ),
        (
            include_str!("fixtures/structure_sh.sh"),
            include_str!("fixtures/structure_sh.shellcheck"),
        ),
        (
            include_str!("fixtures/comparisons.sh"),
            include_str!("fixtures/comparisons.shellcheck"),
        ),
        (
            include_str!("fixtures/control.sh"),
            include_str!("fixtures/control.shellcheck"),
        ),
        (
            include_str!("fixtures/command_shapes.sh"),
            include_str!("fixtures/command_shapes.shellcheck"),
        ),
        (
            include_str!("fixtures/equals.sh"),
            include_str!("fixtures/equals.shellcheck"),
        ),
        (
            include_str!("fixtures/equals_sh.sh"),
            include_str!("fixtures/equals_sh.shellcheck"),
        ),
        (
            include_str!("fixtures/equals_dash.sh"),
            include_str!("fixtures/equals_dash.shellcheck"),
        ),
        (
            include_str!("fixtures/equals_ksh.sh"),
            include_str!("fixtures/equals_ksh.shellcheck"),
        ),
        (
            include_str!("fixtures/command_details.sh"),
            include_str!("fixtures/command_details.shellcheck"),
        ),
        (
            include_str!("fixtures/command_details_sh.sh"),
            include_str!("fixtures/command_details_sh.shellcheck"),
        ),
    ];
    for (script, shellcheck) in fixtures {
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
