use std::fs;

use super::{ModuleFacts, fingerprint, helper, module_files};
use crate::config::GoTarget;
use crate::lint::go::platform::GoPlatform;
use crate::test_support::TempTree;

fn facts_line(file: &str, body: &str) -> String {
    format!(
        "{{\"file\":{},{body}}}\n",
        serde_json::to_string(file).expect("la ruta es texto")
    )
}

#[test]
fn facts_of_the_same_file_from_two_packages_are_merged() {
    let tree = TempTree::new("go-types-merge");
    tree.write("a.go", "package a\n");
    let file = tree.path().join("a.go").display().to_string();
    let lines = [
        facts_line(&file, r#""calls":[{"line":3,"column":7,"names":["os.Remove"],"function":"os.Remove","selector":"os.Remove"}],"receives":null,"ranges":null"#),
        facts_line(&file, r#""calls":null,"receives":[{"line":4,"column":2}],"ranges":[{"line":5,"column":2,"call":null}]"#),
        "no es json\n".to_owned(),
    ]
    .concat();
    let module = ModuleFacts::parse(&lines);
    let facts = module
        .file(&tree.path().join("a.go"))
        .expect("hay hechos del archivo");
    assert_eq!(facts.calls.iter().flatten().count(), 1);
    assert_eq!(facts.receives.iter().flatten().count(), 1);
    assert_eq!(facts.ranges.iter().flatten().count(), 1);
    assert!(module.file(&tree.path().join("b.go")).is_none());
}

#[test]
fn the_fingerprint_follows_the_go_files_the_module_and_the_platform() {
    let tree = TempTree::new("go-types-fingerprint");
    tree.write("go.mod", "module a\n");
    tree.write("a.go", "package a\n");
    tree.write("README.md", "uno\n");
    tree.write(".git/x.go", "package x\n");
    let linux = GoPlatform::default();
    let windows = GoPlatform::of(&GoTarget {
        goos: Some("windows".to_owned()),
        goarch: Some("amd64".to_owned()),
        build_tags: Vec::new(),
    });
    let first = fingerprint(tree.path(), &linux);
    assert_eq!(fingerprint(tree.path(), &linux), first);
    assert_ne!(fingerprint(tree.path(), &windows), first);
    tree.write("README.md", "dos, más largo\n");
    assert_eq!(fingerprint(tree.path(), &linux), first);
    tree.write("a.go", "package a\n\nfunc f() {}\n");
    assert_ne!(fingerprint(tree.path(), &linux), first);
    let mut names: Vec<String> = module_files(tree.path())
        .iter()
        .filter_map(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    assert_eq!(names, ["a.go", "go.mod"]);
}

#[test]
fn without_go_the_helper_reports_why() {
    let tree = TempTree::new("go-types-helper");
    tree.write("go.mod", "module a\n");
    let result = helper::facts(&tree.path().join("cache"), tree.path(), &GoPlatform::default());
    if let Err(detail) = result {
        assert_ne!(detail, String::new());
        assert!(fs::read_dir(tree.path().join("cache")).is_ok());
    }
}
