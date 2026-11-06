use super::{RcFile, disables_something_new, find, parse};
use crate::test_support::TempTree;

#[test]
fn reads_codes_and_shell_between_comments() {
    let text =
        "# comentario\ndisable=SC2086,SC2034 # motivo\n\ndisable=SC1000-SC1100\nshell='bash'\nshell=sh\n";
    assert_eq!(
        parse(".shellcheckrc", text),
        RcFile {
            name: ".shellcheckrc",
            disabled: vec![
                "SC2086".to_owned(),
                "SC2034".to_owned(),
                "SC1000-SC1100".to_owned()
            ],
            shell: Some("bash".to_owned()),
        }
    );
}

#[test]
fn the_nearest_file_wins_and_the_hidden_name_goes_first() {
    let tree = TempTree::new("rc-nearest");
    tree.write(".shellcheckrc", "disable=SC1\n");
    tree.write("a/shellcheckrc", "disable=SC2\n");
    tree.write("a/.shellcheckrc", "disable=SC3\n");
    let nearest = find(&tree.path().join("a/script.sh")).expect("hay un archivo en la carpeta");
    assert_eq!(nearest.disabled, ["SC3"]);
    let upper = find(&tree.path().join("b/script.sh")).expect("hay uno más arriba");
    assert_eq!(upper.disabled, ["SC1"]);
}

#[test]
fn the_search_stops_at_the_repository_root() {
    let tree = TempTree::new("rc-repository");
    tree.write(".shellcheckrc", "disable=SC1\n");
    tree.write("project/.git/HEAD", "ref: refs/heads/main\n");
    assert_eq!(find(&tree.path().join("project/src/script.sh")), None);
}

#[test]
fn only_codes_missing_from_the_previous_version_count_as_new() {
    assert!(disables_something_new("disable=SC1,SC2", "disable=SC1\n"));
    assert!(disables_something_new("disable=SC1", ""));
    assert!(!disables_something_new("disable=SC1", "disable=SC1,SC2\n"));
    assert!(!disables_something_new("# disable=SC3", "disable=SC1\n"));
    assert!(!disables_something_new("shell=bash", ""));
}
