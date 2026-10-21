use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::cache::location::default_directory;

const CLASSPATH_VARIABLE: &str = "KERBO_JAVA_CLASSPATH";
const POM: &str = "pom.xml";
const CLASSPATH_DIRECTORY: &str = "classpath";
const CLASSPATH_EXTENSION: &str = "txt";
const PARTIAL_EXTENSION: &str = "partial";
const SKIPPED_DIRECTORIES: &[&str] = &["target", "node_modules", ".git", "build"];
const MAX_POM_DEPTH: usize = 3;
const LINE_FEED: char = '\n';
const CARRIAGE_RETURN: char = '\r';
// Silencioso, sin red, y con el classpath de cada módulo agregado al mismo archivo.
const MAVEN_ARGUMENTS: [&str; 4] = [
    "-q",
    "-o",
    "dependency:build-classpath",
    "-Dmdep.appendOutput=true",
];
const OUTPUT_FILE_ARGUMENT: &str = "-Dmdep.outputFile=";
#[cfg(windows)]
const MAVEN: &str = "mvn.cmd";
#[cfg(not(windows))]
const MAVEN: &str = "mvn";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// Los jars de las dependencias del proyecto. Se le piden una sola vez a Maven (sin red: lo que
// no esté bajado no está) y se guardan con la huella de los `pom.xml`; si no cambian, no se
// vuelve a preguntar. `KERBO_JAVA_CLASSPATH` los da a mano, para Gradle o cualquier otra cosa.
pub fn for_project(root: &Path) -> Vec<PathBuf> {
    if let Some(given) = env::var_os(CLASSPATH_VARIABLE).filter(|value| !value.is_empty()) {
        return env::split_paths(&given).collect();
    }
    let poms = pom_files(root);
    if poms.is_empty() {
        return Vec::new();
    }
    let Some(directory) = default_directory().map(|cache| cache.join(CLASSPATH_DIRECTORY)) else {
        return Vec::new();
    };
    let saved = directory
        .join(format!("{:016x}", fingerprint(root, &poms)))
        .with_extension(CLASSPATH_EXTENSION);
    if let Ok(contents) = fs::read_to_string(&saved) {
        return jars_in(&contents);
    }
    let contents = ask_maven(root, &saved).unwrap_or_default();
    if fs::create_dir_all(&directory).is_ok() {
        drop(fs::write(&saved, &contents));
    }
    jars_in(&contents)
}

pub(super) fn pom_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    while let Some((directory, depth)) = pending.pop() {
        let pom = directory.join(POM);
        if pom.is_file() {
            found.push(pom);
        }
        if depth >= MAX_POM_DEPTH {
            continue;
        }
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let skipped = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| SKIPPED_DIRECTORIES.contains(&name));
            if path.is_dir() && !skipped {
                pending.push((path, depth.saturating_add(1)));
            }
        }
    }
    found.sort();
    found
}

pub(super) fn fingerprint(root: &Path, poms: &[PathBuf]) -> u64 {
    let mut hasher = DefaultHasher::new();
    root.hash(&mut hasher);
    for pom in poms {
        pom.hash(&mut hasher);
        fs::read(pom).unwrap_or_default().hash(&mut hasher);
    }
    hasher.finish()
}

pub(super) fn ask_maven(root: &Path, saved: &Path) -> Option<String> {
    let partial = saved.with_extension(PARTIAL_EXTENSION);
    fs::create_dir_all(partial.parent()?).ok()?;
    drop(fs::remove_file(&partial));
    let mut command = Command::new(MAVEN);
    command
        .current_dir(root)
        .args(MAVEN_ARGUMENTS)
        .arg(format!("{OUTPUT_FILE_ARGUMENT}{}", partial.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_window(&mut command);
    let finished = command.status().ok()?;
    let contents = fs::read_to_string(&partial).ok();
    drop(fs::remove_file(&partial));
    finished.success().then_some(contents).flatten()
}

#[cfg(windows)]
fn hide_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
const fn hide_window(_command: &mut Command) {}

// Maven escribe cada módulo en una línea, con los jars separados como en el PATH.
pub(super) fn jars_in(contents: &str) -> Vec<PathBuf> {
    let mut jars: Vec<PathBuf> = contents
        .split(LINE_FEED)
        .map(|line| line.trim_end_matches(CARRIAGE_RETURN))
        .filter(|line| !line.is_empty())
        .flat_map(env::split_paths)
        .collect();
    jars.sort();
    jars.dedup();
    jars
}
