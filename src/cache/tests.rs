use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::Parser;

use super::budget::{self, DEFAULT_MAX_BYTES};
use super::codec::{Corrupt, Reader, Writer};
use super::key::CacheKey;
use super::shard::{Entry, Shard};
use super::store::Store;
use super::{AnalysisCache, AnalysisSubject, analysis_codec, location};
use crate::analysis::{FileAnalysis, analyze};
use crate::config::Config;
use crate::language::Language;
use crate::limits::{LimitOverride, Limits, Preset};
use crate::rules::RuleId;
use crate::scan::{ScanOutcome, ScanRequest, scan};
use crate::test_support::TempTree;

const SMALL_DUPLICATE_LIMIT: usize = 5;
const TINY_BUDGET: usize = 100;
const OLD_STAMP: u64 = 10;
const NEW_STAMP: u64 = 20;
const JAVA_METHOD: &str =
    "  int sum(int first, int second) {\n    int total = first + second;\n    return total * total;\n  }\n";
const JAVA_LINTS: &str = "package a;\nimport java.lang.String;\nimport a.Same;\nimport b.C;\nimport b.C;\nimport java.util.*;\nfinal class A {\n  private A() {}\n  final void m(int value) {}\n}\nclass B { private B() {} }\n";
const GO_SOURCE: &str = "package sample\n\nfunc Many(a, b, c, d, e int) int {\n\tif a > b {\n\t\treturn c\n\t}\n\treturn d + e\n}\n";
const PYTHON_SOURCE: &str =
    "def build(first, second, third, fourth, fifth, sixth, seventh):\n    # x = 1\n    return first * 42\n";
const PYTHON_LINTS: &str = "import os\nimport sys\nprint(f\"hola\", sys)\nif sys == None: pass\n";
const RUST_BRANCHES: usize = 40;
const RUST_MANIFEST: &str = "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n";
const RUST_CALLER_PATH: &str = "src/caller.rs";
const RUST_CALLER: &str = "pub fn count(flag: bool) -> usize {\n    let mut total = 0;\n    branches!(flag, total);\n    total\n}\n";
const RUST_COUNTING_BRANCH: &str = "        $total += 1;\n";
const RUST_CONDITIONAL_BRANCH: &str = "        if $flag {\n            $total += 1;\n        }\n";
const TYPESCRIPT_SOURCE: &str = "function pick(flag: boolean, value: number) {\n  if (flag) {\n    value = value + 1;\n    return value * 3;\n  } else {\n    value = value + 1;\n    return value * 3;\n  }\n}\n";

fn analysis_of(language: Language, source: &str, limits: Limits) -> FileAnalysis {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    analyze(&mut parser, language, source.as_bytes(), limits).expect("el archivo se parsea")
}

const fn subject(language: Language, source: &[u8], context: Option<u64>) -> AnalysisSubject<'_> {
    AnalysisSubject {
        language,
        source,
        context,
    }
}

fn strict(language: Language) -> Limits {
    Preset::Strict.limits(language, false)
}

fn write_project(tree: &TempTree) {
    tree.write("src/A.java", &format!("class A {{\n{JAVA_METHOD}}}\n"));
    tree.write("src/B.java", &format!("class B {{\n{JAVA_METHOD}}}\n"));
    tree.write("src/many.go", GO_SOURCE);
    tree.write("src/build.py", PYTHON_SOURCE);
    tree.write("src/pick.ts", TYPESCRIPT_SOURCE);
}

const DUPLICATE_OVERRIDE: [LimitOverride; 1] = [LimitOverride {
    rule: RuleId::DuplicateCode,
    value: SMALL_DUPLICATE_LIMIT,
}];

fn scan_project(root: &Path, cache: Option<&AnalysisCache>) -> ScanOutcome {
    let config = Config::load(None, root).expect("la configuración es válida");
    scan(&ScanRequest {
        roots: &[root.join("src")],
        stacks: &[],
        overrides: &DUPLICATE_OVERRIDE,
        config: &config,
        cache,
    })
}

fn summary(outcome: &ScanOutcome) -> Vec<String> {
    outcome
        .diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{}:{}:{} {} {:?}",
                diagnostic.relative_path,
                diagnostic.line,
                diagnostic.column,
                diagnostic.fingerprint,
                diagnostic.message
            )
        })
        .collect()
}

fn shard_files(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory).map_or_else(
        |_| Vec::new(),
        |entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect()
        },
    )
}

#[test]
fn numbers_texts_and_options_survive_the_round_trip() {
    let mut writer = Writer::default();
    for value in [0, 1, 127, 128, 300, u64::MAX] {
        writer.number(value);
    }
    writer.optional_size(None);
    writer.optional_size(Some(7));
    writer.fixed(u64::MAX - 1);
    writer.text("número");
    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    let numbers: Vec<u64> = (0..6).map(|_| reader.number().expect("se lee")).collect();
    assert_eq!(numbers, [0, 1, 127, 128, 300, u64::MAX]);
    assert_eq!(reader.optional_size(), Ok(None));
    assert_eq!(reader.optional_size(), Ok(Some(7)));
    assert_eq!(reader.fixed(), Ok(u64::MAX - 1));
    assert_eq!(reader.text().as_deref(), Ok("número"));
    assert!(reader.is_empty());
}

#[test]
fn truncated_or_overlong_bytes_are_corrupt() {
    assert_eq!(Reader::new(&[0x80]).number(), Err(Corrupt));
    assert_eq!(Reader::new(&[0xff; 11]).number(), Err(Corrupt));
    assert_eq!(Reader::new(&[2, 1]).optional_size(), Err(Corrupt));
    assert_eq!(Reader::new(&[3]).fixed(), Err(Corrupt));
    assert_eq!(Reader::new(&[2, 0xff, 0xfe]).text(), Err(Corrupt));
    assert_eq!(Reader::new(&[9]).take(2), Err(Corrupt));
}

#[test]
fn every_kind_of_analysis_decodes_to_the_same_value() {
    let samples = [
        (Language::Java, format!("class A {{\n{JAVA_METHOD}}}\n")),
        (Language::Java, JAVA_LINTS.to_owned()),
        (Language::Go, GO_SOURCE.to_owned()),
        (Language::Python, PYTHON_SOURCE.to_owned()),
        (Language::Python, PYTHON_LINTS.to_owned()),
        (Language::TypeScript, TYPESCRIPT_SOURCE.to_owned()),
        (Language::Rust, "fn main() {\n    let total = 1;\n}\n".to_owned()),
    ];
    for (language, source) in samples {
        let analysis = analysis_of(language, &source, strict(language));
        let encoded = analysis_codec::encode(&analysis);
        let decoded = analysis_codec::decode(&encoded).expect("se decodifica");
        assert_eq!(format!("{decoded:?}"), format!("{analysis:?}"), "{language:?}");
        let truncated = encoded.get(..encoded.len() - 1).expect("hay bytes");
        assert!(analysis_codec::decode(truncated).is_err(), "{language:?}");
        let mut extended = encoded.clone();
        extended.push(0);
        assert!(analysis_codec::decode(&extended).is_err(), "{language:?}");
    }
    assert!(analysis_codec::decode(&[0, 0, 0, 0, 0, 0, 9]).is_err());
}

#[test]
fn the_key_changes_with_content_language_and_limits() {
    let tree = TempTree::new("cache-keys");
    let cache = AnalysisCache::open(tree.path().to_path_buf(), DEFAULT_MAX_BYTES);
    let limits = strict(Language::Java);
    let base = cache.analysis_key(subject(Language::Java, b"class A {}", None), limits);
    assert_eq!(
        base,
        cache.analysis_key(subject(Language::Java, b"class A {}", None), limits)
    );
    assert_ne!(
        base,
        cache.analysis_key(subject(Language::Java, b"class B {}", None), limits)
    );
    assert_ne!(
        base,
        cache.analysis_key(subject(Language::Go, b"class A {}", None), limits)
    );
    let looser = limits.with_rule(RuleId::NestingDepth, Some(SMALL_DUPLICATE_LIMIT));
    assert_ne!(
        base,
        cache.analysis_key(subject(Language::Java, b"class A {}", None), looser)
    );
    assert_ne!(
        base,
        cache.analysis_key(subject(Language::Java, b"class A {}", Some(0)), limits)
    );
}

#[test]
fn stored_entries_survive_a_new_process_and_corruption_is_ignored() {
    let tree = TempTree::new("cache-store");
    let key = CacheKey::from_bits(u128::MAX);
    let store = Store::new(tree.path().to_path_buf(), DEFAULT_MAX_BYTES);
    assert_eq!(store.get(key), None);
    store.put(key, b"contenido que se repite, contenido que se repite");
    store.persist();
    let reopened = Store::new(tree.path().to_path_buf(), DEFAULT_MAX_BYTES);
    assert_eq!(
        reopened.get(key).as_deref(),
        Some(b"contenido que se repite, contenido que se repite".as_slice())
    );
    let shard = shard_files(tree.path());
    assert_eq!(shard.len(), 1);
    let mut bytes = fs::read(&shard[0]).expect("se lee el shard");
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    fs::write(&shard[0], &bytes).expect("se escribe el shard");
    assert_eq!(
        Store::new(tree.path().to_path_buf(), DEFAULT_MAX_BYTES).get(key),
        None
    );
    fs::write(&shard[0], b"basura").expect("se escribe el shard");
    assert_eq!(
        Store::new(tree.path().to_path_buf(), DEFAULT_MAX_BYTES).get(key),
        None
    );
}

#[test]
fn two_processes_writing_the_same_shard_keep_both_entries() {
    let tree = TempTree::new("cache-concurrent");
    let first = CacheKey::from_bits(1);
    let second = CacheKey::from_bits(2);
    assert_eq!(first.shard(), second.shard());
    let one_process = Store::new(tree.path().to_path_buf(), DEFAULT_MAX_BYTES);
    let other_process = Store::new(tree.path().to_path_buf(), DEFAULT_MAX_BYTES);
    assert_eq!(one_process.get(first), None);
    assert_eq!(other_process.get(second), None);
    one_process.put(first, b"del primero");
    other_process.put(second, b"del segundo");
    one_process.persist();
    other_process.persist();
    let reopened = Store::new(tree.path().to_path_buf(), DEFAULT_MAX_BYTES);
    assert_eq!(reopened.get(first).as_deref(), Some(b"del primero".as_slice()));
    assert_eq!(reopened.get(second).as_deref(), Some(b"del segundo".as_slice()));
}

#[test]
fn the_size_cap_comes_from_the_variable_then_the_config_then_the_default() {
    let megabytes = |count: usize| count * 1024 * 1024;
    assert_eq!(budget::resolve(Some("64".into()), Some(32)), megabytes(64));
    assert_eq!(budget::resolve(None, Some(32)), megabytes(32));
    assert_eq!(budget::resolve(Some("mucho".into()), Some(32)), megabytes(32));
    assert_eq!(budget::resolve(None, None), DEFAULT_MAX_BYTES);
    assert_eq!(DEFAULT_MAX_BYTES, megabytes(256));
}

#[test]
fn eviction_drops_the_least_recently_used_entries_first() {
    let mut shard = Shard::default();
    let old = CacheKey::from_bits(1);
    let new = CacheKey::from_bits(2);
    shard
        .entries
        .insert(old, Entry::new(OLD_STAMP, vec![0; TINY_BUDGET / 2]));
    shard
        .entries
        .insert(new, Entry::new(NEW_STAMP, vec![0; TINY_BUDGET / 2]));
    shard.evict_to(TINY_BUDGET);
    assert!(shard.dirty && shard.entries.contains_key(&new) && !shard.entries.contains_key(&old));
    let parsed = Shard::parse(&shard.serialize());
    assert_eq!(parsed.entries.len(), 1);
    assert!(parsed.entries.get(&new).is_some_and(Entry::is_intact));
    let mut untouched = Shard::default();
    untouched.evict_to(TINY_BUDGET);
    assert!(!untouched.dirty);
}

#[test]
fn a_cached_scan_reports_exactly_what_a_fresh_one_does() {
    let tree = TempTree::new("cache-scan");
    write_project(&tree);
    let directory = tree.path().join("cache");
    let fresh = summary(&scan_project(tree.path(), None));
    assert!(fresh.iter().any(|line| line.contains("duplicate-code")));
    let cache = AnalysisCache::open(directory.clone(), DEFAULT_MAX_BYTES);
    assert_eq!(summary(&scan_project(tree.path(), Some(&cache))), fresh);
    assert_ne!(shard_files(&directory).len(), 0);
    let warm = AnalysisCache::open(directory.clone(), DEFAULT_MAX_BYTES);
    let limits = strict(Language::Go).with_override(DUPLICATE_OVERRIDE[0]);
    let key = warm.analysis_key(subject(Language::Go, GO_SOURCE.as_bytes(), None), limits);
    assert!(warm.analysis(key).is_some());
    assert_eq!(summary(&scan_project(tree.path(), Some(&warm))), fresh);
    for shard in shard_files(&directory) {
        fs::write(shard, b"KRBC\x01").expect("se rompe el shard");
    }
    let broken = AnalysisCache::open(directory, DEFAULT_MAX_BYTES);
    let outcome = scan_project(tree.path(), Some(&broken));
    assert!(outcome.failures.is_empty());
    assert_eq!(summary(&outcome), fresh);
}

#[test]
fn an_edited_file_is_analyzed_again() {
    let tree = TempTree::new("cache-edit");
    write_project(&tree);
    let directory = tree.path().join("cache");
    let cache = AnalysisCache::open(directory.clone(), DEFAULT_MAX_BYTES);
    let before = summary(&scan_project(tree.path(), Some(&cache)));
    tree.write(
        "src/many.go",
        "package sample\n\nfunc Few(a int) int {\n\treturn a\n}\n",
    );
    let edited = AnalysisCache::open(directory, DEFAULT_MAX_BYTES);
    let after = summary(&scan_project(tree.path(), Some(&edited)));
    assert_ne!(before, after);
    assert_eq!(after, summary(&scan_project(tree.path(), None)));
}

fn rust_crate_root(branch: &str) -> String {
    format!(
        "macro_rules! branches {{\n    ($flag:expr, $total:ident) => {{\n{}    }};\n}}\n\nmod caller;\n",
        branch.repeat(RUST_BRANCHES)
    )
}

#[test]
fn a_rust_file_is_analyzed_again_when_a_macro_of_its_crate_changes() {
    let tree = TempTree::new("cache-macros");
    tree.write("Cargo.toml", RUST_MANIFEST);
    tree.write("src/lib.rs", &rust_crate_root(RUST_COUNTING_BRANCH));
    tree.write(RUST_CALLER_PATH, RUST_CALLER);
    let directory = tree.path().join("cache");
    let cache = AnalysisCache::open(directory.clone(), DEFAULT_MAX_BYTES);
    let before = scan_project(tree.path(), Some(&cache));
    tree.write("src/lib.rs", &rust_crate_root(RUST_CONDITIONAL_BRANCH));
    let edited = AnalysisCache::open(directory, DEFAULT_MAX_BYTES);
    let after = scan_project(tree.path(), Some(&edited));
    assert!(!caller_is_too_complex(&before));
    assert!(caller_is_too_complex(&after));
    assert_eq!(summary(&after), summary(&scan_project(tree.path(), None)));
}

fn caller_is_too_complex(outcome: &ScanOutcome) -> bool {
    outcome.diagnostics.iter().any(|diagnostic| {
        diagnostic.relative_path == RUST_CALLER_PATH && diagnostic.rule == RuleId::CognitiveComplexity
    })
}

#[test]
fn the_default_directory_lives_under_the_user_cache() {
    assert!(location::default_directory().is_some());
    assert!(AnalysisCache::open_default(None).is_some());
}
