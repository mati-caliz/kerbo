use std::cmp::Ordering;

use super::compare;
use crate::osv::Ecosystem;

fn ascending(ecosystem: Ecosystem, versions: &[&str]) {
    for pair in versions.windows(2) {
        let [lower, higher] = pair else {
            continue;
        };
        assert_eq!(
            compare(ecosystem, lower, higher),
            Ordering::Less,
            "{lower} < {higher}"
        );
        assert_eq!(
            compare(ecosystem, higher, lower),
            Ordering::Greater,
            "{higher} > {lower}"
        );
    }
}

#[test]
fn orders_semver_like_npm_cargo_and_go() {
    ascending(
        Ecosystem::Npm,
        &[
            "0.9.0",
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0",
            "1.0.1",
            "1.10.0",
        ],
    );
    assert_eq!(compare(Ecosystem::CratesIo, "1.2", "1.2.0"), Ordering::Equal);
    assert_eq!(
        compare(Ecosystem::Go, "v1.2.3", "1.2.3+incompatible"),
        Ordering::Equal
    );
    ascending(
        Ecosystem::Go,
        &[
            "0.0.0-20200116001909-b77594299b42",
            "0.0.0-20200622213623-75b288015ac9",
            "0.1.0",
        ],
    );
}

#[test]
fn orders_python_versions_like_pep_440() {
    ascending(
        Ecosystem::PyPi,
        &[
            "1.0.dev1",
            "1.0a1",
            "1.0b2",
            "1.0rc1",
            "1.0",
            "1.0.post1",
            "1.1",
            "2020.6.20",
            "1!0.1",
        ],
    );
    assert_eq!(compare(Ecosystem::PyPi, "1.0.0", "1.0"), Ordering::Equal);
    assert_eq!(compare(Ecosystem::PyPi, "1.0-1", "1.0.post1"), Ordering::Equal);
    assert_eq!(compare(Ecosystem::PyPi, "no-es-pep", "1.0"), Ordering::Less);
}

#[test]
fn orders_maven_versions_with_qualifiers() {
    ascending(
        Ecosystem::Maven,
        &[
            "1.0-alpha1",
            "1.0-beta",
            "1.0-m1",
            "1.0-rc1",
            "1.0-SNAPSHOT",
            "1.0",
            "1.0-sp1",
            "1.0.1",
            "1.1",
        ],
    );
    assert_eq!(compare(Ecosystem::Maven, "1.0", "1.0.0-final"), Ordering::Equal);
    assert_eq!(
        compare(Ecosystem::Maven, "4.1.3.RELEASE", "4.1.3"),
        Ordering::Equal
    );
    assert_eq!(compare(Ecosystem::Maven, "1.0-xyz", "1.0-sp"), Ordering::Greater);
}
