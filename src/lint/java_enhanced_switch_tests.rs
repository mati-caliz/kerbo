use super::tests::{lints, strict};
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "import java.util.Map;

class Switches {
    Object copy(Map<String, String> map) {
        switch (map.size()) {
            case 0:
                return null;
            case 1:
                Map.Entry<String, String> entry = map.entrySet().iterator().next();
                return entry.getKey();
            default:
                return map;
        }
    }

    int noDefault(int kind) {
        switch (kind) {
            case 1:
                return 10;
            case 2:
            case 3:
                return 20;
        }
        return 0;
    }

    int missingExit(int kind) {
        switch (kind) {
            case 1:
                return 10;
        }
        System.out.println();
        return 0;
    }

    int tooLong(int kind) {
        switch (kind) {
            case 1:
                System.out.println();
                System.out.println();
                return 1;
            default:
                return 0;
        }
    }

    void breaking(int kind) {
        switch (kind) {
            case 1:
                System.out.println();
                break;
            default:
                return;
        }
    }

    int throwing(int kind) {
        switch (kind) {
            case 1:
                return 1;
            default:
                throw new IllegalArgumentException();
        }
    }
}
";

fn reported(limits: &Limits) -> Vec<usize> {
    lints(SOURCE, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::EnhancedSwitchMigration))
        .map(|finding| finding.line)
        .collect()
}

#[test]
fn reports_an_old_switch_that_returns_in_every_branch() {
    assert_eq!(reported(&strict()), [5, 17, 58]);
}

#[test]
fn needs_release_fourteen() {
    let java11 = strict().with_java(JavaSettings {
        release: Some(11),
        ..JavaSettings::default()
    });
    assert_eq!(reported(&java11), Vec::<usize>::new());
}
