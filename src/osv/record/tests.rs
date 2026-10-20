use super::Record;
use crate::osv::Ecosystem;
use crate::osv::lockfiles::Dependency;
use crate::osv::responses::same_instant;

fn record(json: &str) -> Record {
    serde_json::from_str(json).expect("el registro es JSON válido")
}

fn dependency(ecosystem: Ecosystem, name: &str, version: &str) -> Dependency {
    Dependency {
        ecosystem,
        name: name.to_owned(),
        version: version.to_owned(),
        line: 1,
    }
}

const RANGES: &str = r#"{
  "id": "OSV-1",
  "details": "\nPrimera línea del detalle.\nSegunda.",
  "affected": [
    {"package": {"name": "Django", "ecosystem": "PyPI"},
     "ranges": [
       {"type": "GIT", "events": [{"introduced": "0"}, {"fixed": "abc"}]},
       {"type": "ECOSYSTEM", "events": [{"fixed": "3.2.5"}, {"introduced": "3.0"}, {"introduced": "4.0"}, {"fixed": "4.1.2"}]}
     ]},
    {"package": {"name": "lodash.template", "ecosystem": "npm"},
     "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}, {"last_affected": "4.5.0"}]}]},
    {"package": {"name": "pinned", "ecosystem": "npm"}, "versions": ["1.0.0"]}
  ]
}"#;

#[test]
fn evaluates_ranges_and_lists_like_the_osv_spec() {
    let record = record(RANGES);
    let python = |version| dependency(Ecosystem::PyPi, "django", version);
    assert!(record.affects(&python("3.1")));
    assert!(!record.affects(&python("3.2.5")));
    assert!(!record.affects(&python("2.2")));
    assert!(record.affects(&python("4.0")));
    assert!(!record.affects(&python("4.1.2")));
    assert_eq!(record.fixed_version(&python("3.1")).as_deref(), Some("3.2.5"));
    assert_eq!(record.fixed_version(&python("4.0")).as_deref(), Some("4.1.2"));
    assert_eq!(record.fixed_version(&python("5.0")), None);
    let template = |version| dependency(Ecosystem::Npm, "lodash.template", version);
    assert!(record.affects(&template("4.5.0")));
    assert!(!record.affects(&template("4.5.1")));
    assert!(record.affects(&dependency(Ecosystem::Npm, "pinned", "1.0.0")));
    assert!(!record.affects(&dependency(Ecosystem::Npm, "pinned", "1.0.1")));
    assert!(!record.affects(&dependency(Ecosystem::Go, "django", "3.1")));
    assert_eq!(record.summary(), "Primera línea del detalle.");
}

#[test]
fn withdrawn_records_affect_nothing_and_long_summaries_are_cut() {
    let long = "x".repeat(400);
    let withdrawn = record(&format!(
        r#"{{"id": "OSV-2", "withdrawn": "2024-01-01T00:00:00Z", "summary": "{long}",
           "affected": [{{"package": {{"name": "a", "ecosystem": "npm"}}, "versions": ["1.0.0"]}}]}}"#
    ));
    assert!(!withdrawn.affects(&dependency(Ecosystem::Npm, "a", "1.0.0")));
    assert!(withdrawn.summary().ends_with('…'));
    assert!(withdrawn.summary().chars().count() < long.len());
}

#[test]
fn compares_modification_stamps_down_to_the_microsecond() {
    assert!(same_instant(
        "2026-10-17T02:55:38.085813876Z",
        "2026-10-17T02:55:38.085813Z"
    ));
    assert!(same_instant(
        "2026-10-17T02:55:38Z",
        "2026-10-17T02:55:38.000000Z"
    ));
    assert!(!same_instant("2026-10-17T02:55:38.1Z", "2026-10-17T02:55:38.2Z"));
}
