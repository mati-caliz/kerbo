use std::collections::HashSet;

use tree_sitter::Parser;

use super::{Facts, is_false};
use crate::analysis::{SourceUnit, analyze_unit};
use crate::config::RustTarget;
use crate::language::Language;
use crate::limits::Limits;
use crate::macro_rules::MacroIndex;
use crate::test_support::TempTree;

const LIBRARY: &str = "pub fn mixed(flag: bool) -> u8 {
    #[cfg(feature = \"missing\")]
    if flag {
        return 1;
    }
    #[cfg(any(unix, windows))]
    if flag {
        return 2;
    }
    match flag {
        #[cfg(target_os = \"kerbo-os\")]
        true => 3,
        _ => 0,
    }
}

#[cfg(not(any(unix, windows)))]
pub fn elsewhere(a: u8, b: u8, c: u8, d: u8, e: u8) -> u8 {
    a + b + c + d + e
}

#[cfg(feature = \"std\")]
pub fn with_std(a: u8, b: u8, c: u8, d: u8, e: u8) -> u8 {
    a + b + c + d + e
}

#[cfg(built_by_the_build_script)]
pub fn unknown(a: u8, b: u8, c: u8, d: u8, e: u8) -> u8 {
    a + b + c + d + e
}
";

fn features(names: &[&str]) -> HashSet<String> {
    names.iter().map(ToString::to_string).collect()
}

#[test]
fn evaluates_predicates_with_three_values() {
    let declared = features(&["std"]);
    let known = &Facts {
        features: Some(&declared),
        target: None,
    };
    assert!(is_false("feature = \"missing\"", known));
    assert!(!is_false("feature = \"std\"", known));
    assert!(
        !is_false("feature = \"std\"", &Facts::default()),
        "sin Cargo.toml no se sabe"
    );
    assert!(!is_false("test", known), "en los tests sí compila");
    assert!(is_false("all(test, not(test))", known));
    assert!(is_false("not(any(unix, windows))", known));
    assert!(is_false("target_pointer_width = \"16\"", known));
    assert!(is_false(
        "all(built_by_the_build_script, feature = \"missing\")",
        known
    ));
    assert!(!is_false("built_by_the_build_script", known));
    assert!(!is_false("not(built_by_the_build_script)", known));
    assert!(!is_false(
        "any(built_by_the_build_script, feature = \"missing\")",
        known
    ));
    assert!(!is_false("target_has_atomic = \"64\"", known));
    assert!(is_false("miri", known));
    assert!(!is_false("debug_assertions", known));
}

#[test]
fn code_under_a_false_cfg_is_not_measured() {
    let tree = TempTree::new("cfg-crate");
    tree.write(
        "Cargo.toml",
        "[package]\nname = \"sample\"\n\n[features]\nstd = []\n",
    );
    tree.write("src/lib.rs", LIBRARY);
    tree.write(
        "src/gone.rs",
        "#![cfg(miri)]\npub fn gone(a: u8, b: u8, c: u8, d: u8, e: u8) {}\n",
    );
    let library = tree.path().join("src/lib.rs");
    let macros = MacroIndex::for_one_scan().crate_with(Language::Rust, &library, &RustTarget::default());
    let measured = |path: &str, source: &str| {
        let mut parser = Parser::new();
        parser
            .set_language(&Language::Rust.grammar().expect("tiene gramática"))
            .expect("la gramática carga");
        let unit = SourceUnit {
            language: Language::Rust,
            source: source.as_bytes(),
            path: Some(&tree.path().join(path)),
            crate_macros: macros.as_ref(),
        };
        analyze_unit(&mut parser, unit, &Limits::default())
            .expect("se parsea")
            .functions
    };
    let functions = measured("src/lib.rs", LIBRARY);
    let names: Vec<(&str, Option<usize>)> = functions
        .iter()
        .map(|function| (function.name.as_str(), function.cognitive))
        .collect();
    assert_eq!(
        names,
        [("mixed", Some(2)), ("with_std", Some(1)), ("unknown", Some(1))]
    );
    let gone = "#![cfg(miri)]\npub fn gone(a: u8, b: u8, c: u8, d: u8, e: u8) {}\n";
    assert_eq!(measured("src/gone.rs", gone).len(), 0);
}

#[test]
fn kerbo_toml_picks_the_platform_the_features_and_the_build_script_cfgs() {
    let declared = features(&["std", "alloc"]);
    let target = RustTarget {
        target_os: Some("windows".to_owned()),
        target_arch: Some("aarch64".to_owned()),
        features: Some(vec!["alloc".to_owned()]),
        cfg: vec!["fast_path".to_owned(), "limb_width = \"64\"".to_owned()],
    };
    let facts = &Facts {
        features: Some(&declared),
        target: Some(&target),
    };
    assert!(is_false("unix", facts));
    assert!(!is_false("windows", facts));
    assert!(is_false("target_os = \"linux\"", facts));
    assert!(!is_false("target_arch = \"aarch64\"", facts));
    assert!(is_false("feature = \"std\"", facts));
    assert!(!is_false("feature = \"alloc\"", facts));
    assert!(!is_false("fast_path", facts));
    assert!(is_false("not(fast_path)", facts));
    assert!(is_false("not(limb_width=\"64\")", facts));
    assert!(
        !is_false("not(other_cfg)", facts),
        "lo que no se nombra sigue sin saberse"
    );
}
