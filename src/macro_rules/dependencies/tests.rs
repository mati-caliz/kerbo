use std::path::PathBuf;

use super::{Wanted, compatible, dependencies, locked_versions, numeric_version, registry_root};
use crate::test_support::TempTree;

const LOCKFILE: &str = "[[package]]\nname = \"snapbox\"\nversion = \"0.6.21\"\n\n[[package]]\nname = \"log\"\nversion = \"0.4.22\"\n";

fn downloaded() -> Vec<PathBuf> {
    [
        "snapbox-0.6.21",
        "snapbox-0.6.9",
        "snapbox-1.2.2",
        "snapbox-macros-1.1.0",
        "log-0.4.22",
    ]
    .iter()
    .map(|folder| PathBuf::from("/registry").join(folder))
    .collect()
}

fn chosen(package: &str, requirement: &str, locked: &[(String, String)]) -> Option<String> {
    registry_root(&Wanted { package, requirement }, locked, &downloaded())
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
}

#[test]
fn without_a_lockfile_picks_the_newest_compatible_version_like_cargo() {
    assert_eq!(chosen("snapbox", "0.6.1", &[]).as_deref(), Some("snapbox-0.6.21"));
    assert_eq!(chosen("snapbox", "1", &[]).as_deref(), Some("snapbox-1.2.2"));
    assert_eq!(chosen("snapbox", "", &[]).as_deref(), Some("snapbox-1.2.2"));
    assert_eq!(chosen("snapbox", "2.0", &[]), None);
    assert!(compatible(&numeric_version("0.6.21"), &numeric_version("0.6.1")));
    assert!(!compatible(&numeric_version("0.7.0"), &numeric_version("0.6.1")));
    assert!(!compatible(&numeric_version("0.6.0"), &numeric_version("0.6.1")));
}

#[test]
fn the_lockfile_pins_the_version() {
    let tree = TempTree::new("dependencies-lock");
    tree.write("Cargo.lock", LOCKFILE);
    tree.write("app/Cargo.toml", "[package]\nname = \"app\"\n");
    let locked = locked_versions(&tree.path().join("app"));
    assert!(locked.contains(&("snapbox".to_owned(), "0.6.21".to_owned())));
    let older = [("snapbox".to_owned(), "0.6.9".to_owned())];
    assert_eq!(chosen("snapbox", "0.6", &older).as_deref(), Some("snapbox-0.6.9"));
}

#[test]
fn reads_every_kind_of_dependency_and_follows_local_paths() {
    let tree = TempTree::new("dependencies-manifest");
    tree.write(
        "app/Cargo.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\nmy-helper = { path = \"../helper\" }\nmissing = \"1\"\n\n[target.'cfg(unix)'.dev-dependencies]\nother = { path = \"../other\", package = \"other-real\" }\n",
    );
    tree.write("helper/Cargo.toml", "[package]\nname = \"my-helper\"\n");
    tree.write("other/Cargo.toml", "[package]\nname = \"other-real\"\n");
    let names: Vec<String> = dependencies(&tree.path().join("app"))
        .into_iter()
        .map(|dependency| dependency.name)
        .collect();
    assert_eq!(names, ["my_helper", "other"]);
    assert_eq!(dependencies(&tree.path().join("nothing")), []);
}
