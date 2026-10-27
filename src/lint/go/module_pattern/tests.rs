use super::listed_by_module_pattern;
use crate::test_support::TempTree;

const SOURCE: &str = "package a\n";

#[test]
fn the_module_pattern_skips_testdata_vendor_and_hidden_directories_below_go_mod() {
    let tree = TempTree::new("go-module-pattern");
    tree.write("go.mod", "module a\n");
    tree.write("_examples/go.mod", "module examples\n");
    let files = [
        ("a.go", true),
        ("inner/b.go", true),
        ("testdata/c.go", false),
        ("inner/testdata/deep/d.go", false),
        ("vendor/lib/e.go", false),
        ("_tools/f.go", false),
        (".hidden/g.go", false),
        ("_examples/h.go", true),
        ("_examples/_skip/i.go", false),
    ];
    for (relative, _) in files {
        tree.write(relative, SOURCE);
    }
    let wrong: Vec<&str> = files
        .iter()
        .filter(|(relative, listed)| listed_by_module_pattern(&tree.path().join(relative)) != *listed)
        .map(|(relative, _)| *relative)
        .collect();
    assert_eq!(wrong, Vec::<&str>::new());
}

#[test]
fn without_a_module_nothing_is_skipped() {
    let tree = TempTree::new("go-module-pattern-loose");
    tree.write("testdata/a.go", SOURCE);
    assert!(listed_by_module_pattern(&tree.path().join("testdata/a.go")));
}
