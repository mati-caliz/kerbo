use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::jimage::ModulesImage;
use super::zip::Archive;

const JAVA_HOME: &str = "JAVA_HOME";
const PATH: &str = "PATH";
const RELEASE_FILE: &str = "release";
const VERSION_KEY: &str = "JAVA_VERSION=";
const SYMBOLS_FILE: &str = "lib/ct.sym";
const MODULES_FILE: &str = "lib/modules";
const SIGNATURE_EXTENSION: &str = ".sig";
const CLASS_EXTENSION: &str = ".class";
const LEGACY_VERSION_PREFIX: &str = "1.";
const QUOTE: char = '"';
const VERSION_SEPARATOR: char = '.';
const PATH_SEPARATOR: char = '/';
const FIRST_LETTER_RELEASE: u32 = 10;
const RELEASE_RADIX: u32 = 36;
#[cfg(windows)]
const JAVA_EXECUTABLE: &str = "java.exe";
#[cfg(not(windows))]
const JAVA_EXECUTABLE: &str = "java";

// Las firmas del JDK para el `release` del proyecto. Como `javac --release`: las versiones anteriores
// a la del JDK instalado salen de `ct.sym`, y la suya de `lib/modules`, porque `ct.sym` no la trae.
#[derive(Debug)]
pub enum Jdk {
    Symbols {
        archive: Archive,
        entries: HashMap<String, String>,
    },
    Image(ModulesImage),
}

impl Jdk {
    pub fn locate(release: Option<u32>) -> Option<Self> {
        Self::at(&java_home()?, release)
    }

    pub fn at(home: &Path, release: Option<u32>) -> Option<Self> {
        let own_release = own_release(home)?;
        release
            .filter(|wanted| *wanted < own_release)
            .and_then(|wanted| symbols(home, wanted))
            .or_else(|| ModulesImage::open(&home.join(MODULES_FILE)).map(Self::Image))
    }

    pub fn contains(&self, entry: &str) -> bool {
        match self {
            Self::Symbols { entries, .. } => entries.contains_key(entry),
            Self::Image(image) => image.contains(entry),
        }
    }

    pub fn read(&self, entry: &str) -> Option<Vec<u8>> {
        match self {
            Self::Symbols { archive, entries } => archive.read(entries.get(entry)?),
            Self::Image(image) => image.read(entry),
        }
    }
}

fn java_home() -> Option<PathBuf> {
    env::var_os(JAVA_HOME)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(home_of_java_in_path)
}

fn home_of_java_in_path() -> Option<PathBuf> {
    env::split_paths(&env::var_os(PATH)?)
        .map(|directory| directory.join(JAVA_EXECUTABLE))
        .find(|candidate| candidate.is_file())
        .and_then(|java| java.canonicalize().ok())
        .and_then(|java| java.parent()?.parent().map(Path::to_path_buf))
}

// `JAVA_VERSION="21.0.1"` da 21, y `JAVA_VERSION="1.8.0_402"`, 8.
fn own_release(home: &Path) -> Option<u32> {
    let contents = fs::read_to_string(home.join(RELEASE_FILE)).ok()?;
    let version = contents
        .lines()
        .find_map(|line| line.strip_prefix(VERSION_KEY))?
        .trim()
        .trim_matches(QUOTE);
    version
        .strip_prefix(LEGACY_VERSION_PREFIX)
        .unwrap_or(version)
        .split(VERSION_SEPARATOR)
        .next()?
        .parse()
        .ok()
}

// `ct.sym` agrupa las firmas iguales en una carpeta cuyo nombre lista los releases que las comparten,
// un carácter por release: `CDEFG/java.base/java/io/File.sig` vale del 12 al 16.
fn symbols(home: &Path, release: u32) -> Option<Jdk> {
    let code = release_code(release)?;
    let archive = Archive::open(&home.join(SYMBOLS_FILE)).ok()?;
    let entries: HashMap<String, String> = archive
        .names()
        .filter_map(|name| Some((class_entry(name, code)?, name.to_owned())))
        .collect();
    (!entries.is_empty()).then_some(Jdk::Symbols { archive, entries })
}

fn release_code(release: u32) -> Option<char> {
    char::from_digit(release, RELEASE_RADIX).map(|code| {
        if release < FIRST_LETTER_RELEASE {
            code
        } else {
            code.to_ascii_uppercase()
        }
    })
}

fn class_entry(name: &str, code: char) -> Option<String> {
    let (releases, in_module) = name.split_once(PATH_SEPARATOR)?;
    if !releases.contains(code) {
        return None;
    }
    let (_, signature) = in_module.split_once(PATH_SEPARATOR)?;
    let class = signature.strip_suffix(SIGNATURE_EXTENSION)?;
    Some(format!("{class}{CLASS_EXTENSION}"))
}
