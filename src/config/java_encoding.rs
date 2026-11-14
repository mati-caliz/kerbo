use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::limits::SourceEncoding;

const DECLARATIONS: [&str; 2] = [
    r"<project\.build\.sourceEncoding>\s*([^<\s]+)\s*<",
    r#"encoding\s*=\s*['"]([^'"]+)['"]"#,
];
const LATIN1_NAMES: [&str; 4] = ["iso-8859-1", "iso8859_1", "latin1", "iso-latin-1"];
const ASCII_NAMES: [&str; 3] = ["us-ascii", "ascii", "ascii7"];

static DECLARED: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    DECLARATIONS
        .iter()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .collect()
});

pub fn detected(root: &Path, build_files: &[&str]) -> SourceEncoding {
    build_files
        .iter()
        .filter_map(|build| fs::read_to_string(root.join(build)).ok())
        .find_map(|text| declared(&text))
        .map_or(SourceEncoding::Utf8, |name| by_name(&name))
}

fn declared(text: &str) -> Option<String> {
    DECLARED
        .iter()
        .find_map(|pattern| pattern.captures(text)?.get(1))
        .map(|name| name.as_str().to_lowercase())
}

fn by_name(name: &str) -> SourceEncoding {
    if LATIN1_NAMES.contains(&name) {
        SourceEncoding::Latin1
    } else if ASCII_NAMES.contains(&name) {
        SourceEncoding::Ascii
    } else {
        SourceEncoding::Utf8
    }
}
