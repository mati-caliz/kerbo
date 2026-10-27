mod expression;

use std::path::Path;

use expression::{go_build_expression, plus_build_expression};

const GO_BUILD_PREFIX: &str = "//go:build";
const PLUS_BUILD_PREFIX: &str = "+build";
const LINE_COMMENT: &str = "//";
const BLOCK_COMMENT_START: &str = "/*";
const BLOCK_COMMENT_END: &str = "*/";
const IGNORED_NAME_PREFIXES: &[char] = &['_', '.'];
const NAME_SEPARATOR: char = '_';
const EXTENSION_SEPARATOR: char = '.';
const TEST_SUFFIX: &str = "test";
const KNOWN_OS: &[&str] = &[
    "aix",
    "android",
    "darwin",
    "dragonfly",
    "freebsd",
    "hurd",
    "illumos",
    "ios",
    "js",
    "linux",
    "nacl",
    "netbsd",
    "openbsd",
    "plan9",
    "solaris",
    "wasip1",
    "windows",
    "zos",
];
const KNOWN_ARCH: &[&str] = &[
    "386",
    "amd64",
    "amd64p32",
    "arm",
    "armbe",
    "arm64",
    "arm64be",
    "loong64",
    "mips",
    "mipsle",
    "mips64",
    "mips64le",
    "mips64p32",
    "mips64p32le",
    "ppc",
    "ppc64",
    "ppc64le",
    "riscv",
    "riscv64",
    "s390",
    "s390x",
    "sparc",
    "sparc64",
    "wasm",
];
// Lo que `go/build` da por cierto en linux/amd64 con cgo y el compilador gc del Go 1.27 de
// golangci-lint v2.15.0, con sus experimentos prendidos por defecto.
const PLATFORM_TAGS: &[&str] = &[
    "linux",
    "amd64",
    "unix",
    "cgo",
    "gc",
    "amd64.v1",
    "goexperiment.regabiwrappers",
    "goexperiment.regabiargs",
    "goexperiment.dwarf5",
    "goexperiment.jsonv2",
    "goexperiment.greenteagc",
    "goexperiment.randomizedheapbase64",
    "goexperiment.sizespecializedmalloc",
];
const RELEASE_TAG_PREFIX: &str = "go1.";
const LATEST_GO_MINOR: u32 = 27;

pub fn file_name_builds(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_none_or(name_builds)
}

fn name_builds(name: &str) -> bool {
    if name.starts_with(IGNORED_NAME_PREFIXES) {
        return false;
    }
    let stem = name.split(EXTENSION_SEPARATOR).next().unwrap_or(name);
    let Some(separator) = stem.find(NAME_SEPARATOR) else {
        return true;
    };
    let mut parts: Vec<&str> = stem
        .get(separator..)
        .unwrap_or_default()
        .split(NAME_SEPARATOR)
        .collect();
    if parts.last() == Some(&TEST_SUFFIX) {
        parts.pop();
    }
    match parts.as_slice() {
        [.., os, arch] if KNOWN_OS.contains(os) && KNOWN_ARCH.contains(arch) => {
            matches_tag(os) && matches_tag(arch)
        }
        [.., last] if KNOWN_OS.contains(last) || KNOWN_ARCH.contains(last) => matches_tag(last),
        _ => true,
    }
}

fn matches_tag(tag: &str) -> bool {
    PLATFORM_TAGS.contains(&tag)
        || tag.strip_prefix(RELEASE_TAG_PREFIX).is_some_and(|minor| {
            minor
                .parse::<u32>()
                .is_ok_and(|number| number.to_string() == minor && (1..=LATEST_GO_MINOR).contains(&number))
        })
}

// `shouldBuild` de `go/build`: manda el `//go:build`; sin él, las líneas `// +build` de la cabecera
// se combinan con `&&`. Una restricción que no se entiende no descarta el archivo.
pub fn header_builds(source: &[u8]) -> bool {
    let text = String::from_utf8_lossy(source);
    let header = file_header(&text);
    if header.go_build_lines > 1 {
        return true;
    }
    if let Some(line) = header.go_build {
        return go_build_expression(line.get(GO_BUILD_PREFIX.len()..).unwrap_or_default())
            .is_none_or(|expression| expression.evaluate(&matches_tag));
    }
    header
        .trimmed
        .lines()
        .filter_map(plus_build_text)
        .map(plus_build_expression)
        .all(|expression| expression.evaluate(&matches_tag))
}

fn plus_build_text(line: &str) -> Option<&str> {
    let comment = line.trim().strip_prefix(LINE_COMMENT)?;
    let rest = comment.trim().strip_prefix(PLUS_BUILD_PREFIX)?;
    let trimmed = rest.trim();
    (rest.is_empty() || trimmed.len() < rest.len()).then_some(trimmed)
}

fn is_go_build_comment(line: &str) -> bool {
    line.strip_prefix(GO_BUILD_PREFIX)
        .is_some_and(|rest| rest.is_empty() || rest.trim_start().len() < rest.len())
}

#[derive(Debug, Default)]
struct FileHeader<'source> {
    trimmed: &'source str,
    go_build: Option<&'source str>,
    go_build_lines: usize,
}

// `parseFileHeader`: los comentarios y líneas en blanco antes del primer código. Las `// +build`
// cuentan sólo hasta la última línea en blanco antes del código; el `//go:build`, en cualquier parte.
fn file_header(text: &str) -> FileHeader<'_> {
    let mut header = FileHeader::default();
    let mut end = 0_usize;
    let mut ended = false;
    let mut in_block_comment = false;
    let mut consumed = 0_usize;
    for raw_line in text.split_inclusive('\n') {
        consumed = consumed.saturating_add(raw_line.len());
        let line = raw_line.trim();
        if line.is_empty() && !ended {
            end = consumed;
            continue;
        }
        if !line.starts_with(LINE_COMMENT) {
            ended = true;
        }
        if !in_block_comment && is_go_build_comment(line) {
            header.go_build_lines = header.go_build_lines.saturating_add(1);
            header.go_build = header.go_build.or(Some(line));
        }
        if !only_comments(line, &mut in_block_comment) {
            break;
        }
    }
    header.trimmed = text.get(..end).unwrap_or_default();
    header
}

fn only_comments(line: &str, in_block_comment: &mut bool) -> bool {
    let mut rest = line;
    while !rest.is_empty() {
        if *in_block_comment {
            let Some(close) = rest.find(BLOCK_COMMENT_END) else {
                return true;
            };
            *in_block_comment = false;
            rest = rest
                .get(close.saturating_add(BLOCK_COMMENT_END.len())..)
                .unwrap_or_default()
                .trim();
        } else if rest.starts_with(LINE_COMMENT) {
            return true;
        } else if let Some(after) = rest.strip_prefix(BLOCK_COMMENT_START) {
            *in_block_comment = true;
            rest = after.trim();
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
