use super::tests::{lints, strict};
use crate::rules::{JavaRule, RuleId};

// Los archivos del proyecto de prueba que se midió con Qodana (`scripts/parity/java/fixtures/api-nueva`):
// las líneas esperadas son las que marcó Qodana 2026.3.
const SWITCHES: &str =
    include_str!("../../scripts/parity/java/fixtures/api-nueva/src/main/java/fixture/Switches.java");
const FACTORIES: &str =
    include_str!("../../scripts/parity/java/fixtures/api-nueva/src/main/java/fixture/Factories.java");
const REPEATS: &str =
    include_str!("../../scripts/parity/java/fixtures/api-nueva/src/main/java/fixture/Repeats.java");

fn lines(source: &str, rule: JavaRule) -> Vec<usize> {
    lints(source, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(rule))
        .map(|finding| finding.line)
        .collect()
}

#[test]
fn a_switch_that_assigns_the_same_local_in_every_branch_matches_qodana() {
    assert_eq!(
        lines(SWITCHES, JavaRule::EnhancedSwitchMigration),
        [6, 21, 49, 84, 122]
    );
}

#[test]
fn collections_built_by_hand_match_qodana() {
    assert_eq!(
        lines(FACTORIES, JavaRule::Java9CollectionFactory),
        [20, 34, 52, 62, 68, 82, 87]
    );
}

#[test]
fn counting_loops_that_append_match_qodana() {
    assert_eq!(
        lines(REPEATS, JavaRule::StringRepeatCanBeUsed),
        [6, 14, 22, 30, 38, 46]
    );
}
