use crate::deploy::sql::violations;

const FIXTURES: [(&str, &str, &str); 5] = [
    (
        "types",
        include_str!("../fixtures/types.sql"),
        include_str!("../fixtures/types.squawk"),
    ),
    (
        "constraints",
        include_str!("../fixtures/constraints.sql"),
        include_str!("../fixtures/constraints.squawk"),
    ),
    (
        "defaults",
        include_str!("../fixtures/defaults.sql"),
        include_str!("../fixtures/defaults.squawk"),
    ),
    (
        "bans",
        include_str!("../fixtures/bans.sql"),
        include_str!("../fixtures/bans.squawk"),
    ),
    (
        "names",
        include_str!("../fixtures/names.sql"),
        include_str!("../fixtures/names.squawk"),
    ),
];

fn sorted(lines: impl Iterator<Item = String>) -> Vec<String> {
    let mut lines: Vec<String> = lines.collect();
    lines.sort();
    lines
}

#[test]
fn every_fixture_matches_squawk() {
    for (name, source, expected) in FIXTURES {
        let found = sorted(violations(source, 1).into_iter().map(|violation| {
            format!(
                "{}:{} {}",
                violation.line,
                violation.column,
                violation.rule.as_str()
            )
        }));
        let expected = sorted(expected.lines().map(str::to_owned));
        assert_eq!(found, expected, "{name}");
    }
}

#[test]
fn unknown_rule_names_in_an_ignore_are_reported_where_they_are_written() {
    let found: Vec<(usize, usize, String)> = violations(
        "-- squawk-ignore ban-drop-column, ban-drop-colum\nSELECT 1;\n/* squawk-ignore-file nope */\n",
        1,
    )
    .into_iter()
    .map(|violation| (violation.line, violation.column, violation.subject))
    .collect();
    assert_eq!(
        found,
        [(1, 35, "ban-drop-colum".to_owned()), (3, 23, "nope".to_owned())]
    );
}
