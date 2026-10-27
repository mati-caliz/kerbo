use std::env;

use crate::config::GoTarget;

const OS_VARIABLE: &str = "GOOS";
const ARCH_VARIABLE: &str = "GOARCH";
const CGO_VARIABLE: &str = "CGO_ENABLED";
const CGO_ON: &str = "1";
const CGO_OFF: &str = "0";
const DEFAULT_OS: &str = "linux";
const DEFAULT_ARCH: &str = "amd64";
const CGO_TAG: &str = "cgo";
const UNIX_TAG: &str = "unix";
const COMPILER_TAG: &str = "gc";
const AMD64_LEVEL_TAG: &str = "amd64.v1";
const RELEASE_TAG_PREFIX: &str = "go1.";
const LATEST_GO_MINOR: u32 = 27;
const UNIX_OS: &[&str] = &[
    "aix",
    "android",
    "darwin",
    "dragonfly",
    "freebsd",
    "hurd",
    "illumos",
    "ios",
    "linux",
    "netbsd",
    "openbsd",
    "solaris",
];
// Los sistemas que además cumplen el tag de otro: android es linux, illumos es solaris, ios es darwin.
const IMPLIED_OS: &[(&str, &str)] = &[("android", "linux"), ("illumos", "solaris"), ("ios", "darwin")];
// Los experimentos que el Go 1.27 de golangci-lint v2.15.0 prende por defecto.
const EXPERIMENT_TAGS: &[&str] = &[
    "goexperiment.regabiwrappers",
    "goexperiment.regabiargs",
    "goexperiment.dwarf5",
    "goexperiment.jsonv2",
    "goexperiment.greenteagc",
    "goexperiment.randomizedheapbase64",
    "goexperiment.sizespecializedmalloc",
];
// Los nombres de Rust para el sistema y la arquitectura de la máquina, con los de Go.
const HOST_OS_NAMES: &[(&str, &str)] = &[("macos", "darwin")];
const HOST_ARCH_NAMES: &[(&str, &str)] = &[
    ("x86_64", "amd64"),
    ("x86", "386"),
    ("aarch64", "arm64"),
    ("loongarch64", "loong64"),
    ("powerpc64", "ppc64"),
];

// La plataforma con que `go/build` decide qué archivos entran: sistema, arquitectura, cgo y los
// tags que pide el repo.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GoPlatform {
    os: String,
    arch: String,
    cgo: bool,
    build_tags: Vec<String>,
}

impl Default for GoPlatform {
    fn default() -> Self {
        Self {
            os: DEFAULT_OS.to_owned(),
            arch: DEFAULT_ARCH.to_owned(),
            cgo: true,
            build_tags: Vec::new(),
        }
    }
}

impl GoPlatform {
    // `kerbo.toml` manda; si no, `GOOS` y `GOARCH`, y si tampoco, la máquina. Como `go/build`, cgo
    // va prendido salvo que se compile para otra plataforma o `CGO_ENABLED` diga otra cosa.
    pub fn of(target: &GoTarget) -> Self {
        let host = (
            host_name(env::consts::OS, HOST_OS_NAMES),
            host_name(env::consts::ARCH, HOST_ARCH_NAMES),
        );
        let os = target
            .goos
            .clone()
            .or_else(|| env::var(OS_VARIABLE).ok())
            .unwrap_or_else(|| host.0.clone());
        let arch = target
            .goarch
            .clone()
            .or_else(|| env::var(ARCH_VARIABLE).ok())
            .unwrap_or_else(|| host.1.clone());
        let cgo = match env::var(CGO_VARIABLE).ok().as_deref() {
            Some(CGO_ON) => true,
            Some(CGO_OFF) => false,
            _ => (os.as_str(), arch.as_str()) == (host.0.as_str(), host.1.as_str()),
        };
        Self {
            os,
            arch,
            cgo,
            build_tags: target.build_tags.clone(),
        }
    }

    pub fn os(&self) -> &str {
        &self.os
    }

    pub fn arch(&self) -> &str {
        &self.arch
    }

    pub const fn cgo(&self) -> bool {
        self.cgo
    }

    pub fn build_tags(&self) -> &[String] {
        &self.build_tags
    }

    // `matchTag` de `go/build`.
    pub fn matches(&self, tag: &str) -> bool {
        tag == self.os
            || tag == self.arch
            || tag == COMPILER_TAG
            || (self.cgo && tag == CGO_TAG)
            || (tag == UNIX_TAG && UNIX_OS.contains(&self.os.as_str()))
            || IMPLIED_OS
                .iter()
                .any(|(os, implied)| self.os == *os && tag == *implied)
            || (self.arch == DEFAULT_ARCH && tag == AMD64_LEVEL_TAG)
            || EXPERIMENT_TAGS.contains(&tag)
            || self.build_tags.iter().any(|build_tag| build_tag == tag)
            || is_release_tag(tag)
    }
}

fn is_release_tag(tag: &str) -> bool {
    tag.strip_prefix(RELEASE_TAG_PREFIX).is_some_and(|minor| {
        minor
            .parse::<u32>()
            .is_ok_and(|number| number.to_string() == minor && (1..=LATEST_GO_MINOR).contains(&number))
    })
}

fn host_name(rust_name: &str, names: &[(&str, &str)]) -> String {
    names
        .iter()
        .find(|(rust, _)| *rust == rust_name)
        .map_or(rust_name, |(_, go)| go)
        .to_owned()
}
