use std::path::Path;

use super::expression::{Expression, go_build_expression};
use super::{file_name_builds, header_builds};
use crate::config::GoTarget;
use crate::lint::go::platform::GoPlatform;

const PACKAGE: &str = "package bc\n";

// Lo que `go list` de Go 1.27 (la imagen de golangci-lint v2.15.0) dejó dentro y fuera del paquete.
const VERDICTS: &[(&str, &str, bool)] = &[
    ("plain.go", "", true),
    ("linux.go", "", true),
    ("windows.go", "", true),
    ("x_windows.go", "", false),
    ("x_linux.go", "", true),
    ("x_amd64.go", "", true),
    ("x_arm64.go", "", false),
    ("x_linux_amd64.go", "", true),
    ("x_linux_arm64.go", "", false),
    ("x_windows_amd64.go", "", false),
    ("x_windows_test.go", "", false),
    ("x_linux_test.go", "", true),
    ("x_unix.go", "", true),
    ("x_foo_windows_bar.go", "", true),
    ("_hidden.go", "", false),
    (".hidden.go", "", false),
    ("ignore_tag.go", "//go:build ignore\n\n", false),
    ("tools_tag.go", "//go:build tools\n\n", false),
    ("not_windows.go", "//go:build !windows\n\n", false),
    ("unix_tag.go", "//go:build unix\n\n", true),
    ("cgo_tag.go", "//go:build cgo && linux\n\n", true),
    ("no_cgo.go", "//go:build !cgo\n\n", false),
    ("go_old.go", "//go:build go1.18\n\n", true),
    ("go_future.go", "//go:build go1.99\n\n", false),
    ("not_go_old.go", "//go:build !go1.20\n\n", false),
    ("paren.go", "//go:build (linux || darwin) && !arm64\n\n", true),
    ("plus_ok.go", "// +build linux darwin\n\n", true),
    ("plus_no.go", "// +build windows\n\n", false),
    ("plus_and.go", "// +build linux,!amd64\n\n", false),
    ("plus_two.go", "// +build linux\n// +build ignore\n\n", false),
    ("plus_no_blank.go", "// +build ignore\n", true),
    (
        "plus_after_doc.go",
        "// Copyright\n\n// +build ignore\n\n// doc\n",
        false,
    ),
    ("gobuild_wins.go", "//go:build linux\n// +build ignore\n\n", true),
    (
        "gobuild_late.go",
        "// Copyright\n\n// doc text\n//go:build ignore\n",
        false,
    ),
    ("block_then.go", "/* license */\n\n//go:build ignore\n\n", false),
    ("gobuildx.go", "//go:buildx ignore\n\n", true),
    ("experiment.go", "//go:build goexperiment.jsonv2\n\n", true),
    ("experiment_off.go", "//go:build goexperiment.arenas\n\n", false),
    ("amdv.go", "//go:build amd64.v1 && gc\n\n", true),
    ("gccgo.go", "//go:build gccgo\n\n", false),
    ("plus_bang.go", "// +build !!linux\n\n", false),
    ("boring.go", "//go:build boringcrypto\n\n", false),
];

#[test]
fn file_names_and_headers_build_like_go_list_on_linux_amd64() {
    let linux = GoPlatform::default();
    let wrong: Vec<&str> = VERDICTS
        .iter()
        .filter(|(name, header, expected)| {
            let source = format!("{header}{PACKAGE}");
            (file_name_builds(Path::new(name), &linux) && header_builds(source.as_bytes(), &linux))
                != *expected
        })
        .map(|(name, _, _)| *name)
        .collect();
    assert_eq!(wrong, Vec::<&str>::new());
}

#[test]
fn constraints_that_do_not_parse_keep_the_file() {
    for header in [
        "//go:build linux &&\n\n",
        "//go:build !!linux\n\n",
        "//go:build a\n//go:build b\n\n",
    ] {
        let source = format!("{header}{PACKAGE}");
        assert!(
            header_builds(source.as_bytes(), &GoPlatform::default()),
            "{header}"
        );
    }
}

#[test]
fn go_build_expressions_keep_precedence_and_parentheses() {
    let tag = |name: &str| Box::new(Expression::Tag(name.to_owned()));
    assert_eq!(
        go_build_expression("a || b && !(c)"),
        Some(Expression::Or(
            tag("a"),
            Box::new(Expression::And(tag("b"), Box::new(Expression::Not(tag("c")))))
        ))
    );
    assert_eq!(go_build_expression("(a"), None);
    assert_eq!(go_build_expression("a & b"), None);
    assert_eq!(go_build_expression("a b"), None);
}

#[test]
fn the_configured_platform_and_build_tags_decide_like_go_list() {
    let target = GoTarget {
        goos: Some("windows".to_owned()),
        goarch: Some("arm64".to_owned()),
        build_tags: vec!["integration".to_owned()],
    };
    let windows = GoPlatform::of(&target);
    let verdict = |name: &str, header: &str| {
        let source = format!("{header}{PACKAGE}");
        file_name_builds(Path::new(name), &windows) && header_builds(source.as_bytes(), &windows)
    };
    assert!(verdict("x_windows_arm64.go", ""));
    assert!(!verdict("x_linux.go", ""));
    assert!(!verdict("x_amd64.go", ""));
    assert!(verdict(
        "tagged.go",
        "//go:build integration && !unix

"
    ));
    assert!(!verdict(
        "cgo.go",
        "//go:build cgo

"
    ));
    assert!(!verdict(
        "level.go",
        "//go:build amd64.v1

"
    ));
}
