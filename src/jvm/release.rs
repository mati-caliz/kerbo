use std::fs;
use std::path::Path;

use super::classpath::{files_named, pom_files};
use super::gradle::BUILD_FILES;

const POM_PROPERTIES: [&str; 7] = [
    "<maven.compiler.release>",
    "<release>",
    "<java.version>",
    "<maven.compiler.target>",
    "<maven.compiler.source>",
    "<target>",
    "<source>",
];
const PROPERTY_START: &str = "${";
const PROPERTY_END: char = '}';
const TAG_OPEN: char = '<';
const TAG_CLOSE: char = '>';
const GRADLE_SETTINGS: [&str; 4] = [
    "JavaLanguageVersion.of(",
    "options.release = ",
    "sourceCompatibility = ",
    "VERSION_",
];
const LEGACY_VERSION_PREFIX: &str = "1.";
const VERSION_DECORATION: [char; 3] = ['\'', '"', '_'];

// El `release` de Java que dice el build del proyecto. Si no lo dice, o lo hereda de un padre que no
// está en el repo, vale la versión del JDK instalado.
pub fn project_release(root: &Path) -> Option<u32> {
    let poms = pom_files(root);
    if poms.is_empty() {
        return first_setting(&files_named(root, BUILD_FILES), &GRADLE_SETTINGS);
    }
    first_setting(&poms, &POM_PROPERTIES)
}

// Las propiedades van por prioridad: `<release>${java.version}</release>` no es un número, así que
// se sigue con la siguiente, que es la que tiene el valor.
fn first_setting(files: &[std::path::PathBuf], settings: &[&str]) -> Option<u32> {
    let contents: Vec<String> = files
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok())
        .collect();
    settings.iter().find_map(|setting| {
        contents.iter().find_map(|content| {
            version_after(content, setting).or_else(|| property_after(&contents, content, setting))
        })
    })
}

// `<source>${jdkVersion}</source>` toma el valor de `<jdkVersion>`, en cualquiera de los archivos.
fn property_after(contents: &[String], content: &str, marker: &str) -> Option<u32> {
    let (_, rest) = content.split_once(marker)?;
    let name = rest.strip_prefix(PROPERTY_START)?.split(PROPERTY_END).next()?;
    let property = format!("{TAG_OPEN}{name}{TAG_CLOSE}");
    contents.iter().find_map(|other| version_after(other, &property))
}

fn version_after(content: &str, marker: &str) -> Option<u32> {
    let (_, rest) = content.split_once(marker)?;
    let written = rest.trim_start_matches(VERSION_DECORATION);
    let version = written.strip_prefix(LEGACY_VERSION_PREFIX).unwrap_or(written);
    let digits = version
        .find(|letter: char| !letter.is_ascii_digit())
        .map_or(version, |end| version.get(..end).unwrap_or(version));
    digits.parse().ok()
}
