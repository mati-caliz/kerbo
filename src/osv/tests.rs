use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{AuditRequest, Ecosystem, OsvMode, Transport, audit, download, normalized_name};
use crate::config::Config;
use crate::i18n::{Message, OsvProblem, Problem};
use crate::rules::{Diagnostic, RuleId};
use crate::test_support::TempTree;

const BATCH: &str = include_str!("testdata/querybatch.json");
const REDOS: &str = include_str!("testdata/GHSA-29mw-wpgm-hmr9.json");
const INJECTION: &str = include_str!("testdata/GHSA-35jh-r3h4-6jhm.json");
const LOCKFILE: &str = r#"{
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "app" },
    "node_modules/lodash": {
      "version": "4.17.20"
    },
    "node_modules/left-pad": {
      "version": "1.3.0"
    }
  }
}"#;
const DEFLATE_LEVEL: u8 = 6;

#[derive(Debug, Default)]
struct Recorded {
    archive: Vec<u8>,
    offline: bool,
}

impl Transport for Recorded {
    fn post_json(&self, _url: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        if self.offline {
            return Err("sin red".to_owned());
        }
        let request: serde_json::Value = serde_json::from_slice(body).map_err(|error| error.to_string())?;
        let recorded: serde_json::Value = serde_json::from_str(BATCH).map_err(|error| error.to_string())?;
        let lodash = recorded["results"][0].clone();
        let results: Vec<serde_json::Value> = request["queries"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|query| {
                if query["package"]["name"] == "lodash" {
                    lodash.clone()
                } else {
                    serde_json::json!({})
                }
            })
            .collect();
        Ok(serde_json::json!({ "results": results }).to_string().into_bytes())
    }

    fn get_all(&self, urls: &[String]) -> Result<Vec<Vec<u8>>, String> {
        let records: HashMap<&str, &str> =
            [("GHSA-29mw-wpgm-hmr9", REDOS), ("GHSA-35jh-r3h4-6jhm", INJECTION)].into();
        urls.iter()
            .map(|url| {
                let id = url.rsplit('/').next().unwrap_or_default();
                records
                    .get(id)
                    .map(|record| record.as_bytes().to_vec())
                    .ok_or_else(|| id.to_owned())
            })
            .collect()
    }

    fn download(&self, _url: &str, destination: &Path) -> Result<(), String> {
        fs::write(destination, &self.archive).map_err(|error| error.to_string())
    }
}

fn run(tree: &TempTree, mode: OsvMode, transport: &Recorded) -> Result<Vec<Diagnostic>, Problem> {
    let config = Config::load(None, tree.path())?;
    audit(&AuditRequest {
        roots: &[tree.path().to_path_buf()],
        config: &config,
        mode,
        transport,
        directory: Some(tree.path().join("cache")),
    })
}

fn advisories(diagnostics: &[Diagnostic]) -> Vec<(String, usize, Option<String>)> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.rule, RuleId::VulnerableDependency);
            let Message::VulnerableDependency(advisory) = &diagnostic.message else {
                panic!("se esperaba una vulnerabilidad: {diagnostic:?}");
            };
            assert!(advisory.aliases.iter().any(|alias| alias.starts_with("CVE-")));
            (advisory.id.clone(), diagnostic.line, advisory.fixed.clone())
        })
        .collect()
}

fn expected() -> Vec<(String, usize, Option<String>)> {
    let fixed = Some("4.17.21".to_owned());
    vec![
        ("GHSA-29mw-wpgm-hmr9".to_owned(), 5, fixed.clone()),
        ("GHSA-35jh-r3h4-6jhm".to_owned(), 5, fixed),
    ]
}

#[test]
fn finds_vulnerable_dependencies_online_and_caches_the_answers() {
    let tree = TempTree::new("osv-online");
    tree.write("package-lock.json", LOCKFILE);
    let found = run(&tree, OsvMode::Online, &Recorded::default()).expect("la consulta funciona");
    assert_eq!(advisories(&found), expected());
    assert!(
        found[0]
            .fingerprint
            .starts_with("vulnerable-dependency:package-lock.json:lodash@4.17.20:")
    );
    let offline = Recorded {
        offline: true,
        ..Recorded::default()
    };
    let cached = run(&tree, OsvMode::Online, &offline).expect("la caché responde sin red");
    assert_eq!(advisories(&cached), expected());
    tree.write("package-lock.json", &LOCKFILE.replace("4.17.20", "4.17.19"));
    assert!(matches!(
        run(&tree, OsvMode::Online, &offline),
        Err(Problem::Osv(OsvProblem::Unavailable(_)))
    ));
}

#[test]
fn finds_the_same_with_the_downloaded_database() {
    let tree = TempTree::new("osv-offline");
    tree.write("package-lock.json", LOCKFILE);
    let transport = Recorded {
        archive: zip(&[
            ("a.json", REDOS, true),
            ("b.json", INJECTION, false),
            ("c.json", "{", true),
        ]),
        offline: true,
    };
    let notices =
        download(&[Ecosystem::Npm], &transport, Some(tree.path().join("cache"))).expect("se baja la base");
    assert_eq!(notices.len(), 1);
    let found = run(&tree, OsvMode::Offline, &transport).expect("la base local responde");
    assert_eq!(advisories(&found), expected());
    tree.write("requirements.txt", "django==4.2.1\n");
    assert!(matches!(
        run(&tree, OsvMode::Offline, &transport),
        Err(Problem::Osv(OsvProblem::DatabaseMissing(_)))
    ));
}

#[test]
fn skips_projects_without_lockfiles_and_ignored_paths() {
    let tree = TempTree::new("osv-empty");
    tree.write("src/main.rs", "fn main() {}\n");
    let offline = Recorded {
        offline: true,
        ..Recorded::default()
    };
    assert!(
        run(&tree, OsvMode::Online, &offline)
            .expect("no hace falta red")
            .is_empty()
    );
    tree.write("vendor/package-lock.json", LOCKFILE);
    tree.write(
        "kerbo.toml",
        "[[exceptions]]\npaths = [\"vendor/**\"]\nrule = \"vulnerable-dependency\"\nreason = \"código de terceros\"\n",
    );
    assert!(
        run(&tree, OsvMode::Online, &offline)
            .expect("no hace falta red")
            .is_empty()
    );
}

#[test]
fn checksums_already_declared_in_the_module_file_are_not_repeated() {
    let tree = TempTree::new("osv-go");
    tree.write("go.mod", "module app\n\nrequire github.com/a/b v1.0.0\n");
    tree.write(
        "go.sum",
        "github.com/a/b v1.0.0 h1:x=\ngithub.com/c/d v2.0.0 h1:y=\n",
    );
    let config = Config::load(None, tree.path()).expect("la configuración es válida");
    let lockfiles = super::read_lockfiles(&[tree.path().to_path_buf()], &config);
    let mut names: Vec<(String, Vec<String>)> = lockfiles
        .into_iter()
        .map(|(identity, dependencies)| {
            let names = dependencies
                .into_iter()
                .map(|dependency| dependency.name)
                .collect();
            (identity.relative, names)
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            ("go.mod".to_owned(), vec!["github.com/a/b".to_owned()]),
            ("go.sum".to_owned(), vec!["github.com/c/d".to_owned()]),
        ]
    );
}

#[test]
fn normalizes_python_names_like_pep_503() {
    assert_eq!(
        normalized_name(Ecosystem::PyPi, "Flask_SQLAlchemy"),
        "flask-sqlalchemy"
    );
    assert_eq!(
        normalized_name(Ecosystem::PyPi, "zope..interface"),
        "zope-interface"
    );
    assert_eq!(normalized_name(Ecosystem::Npm, "Left_Pad"), "Left_Pad");
    assert_eq!(
        Ecosystem::ALL.map(Ecosystem::as_str),
        ["crates.io", "npm", "Go", "PyPI", "Maven"]
    );
}

fn zip(files: &[(&str, &str, bool)]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut central = Vec::new();
    for (name, body, deflate) in files {
        let offset = u32::try_from(archive.len()).expect("el zip de prueba es chico");
        let data = if *deflate {
            miniz_oxide::deflate::compress_to_vec(body.as_bytes(), DEFLATE_LEVEL)
        } else {
            body.as_bytes().to_vec()
        };
        let method: u16 = if *deflate { 8 } else { 0 };
        let size = u32::try_from(data.len()).expect("entra en 32 bits");
        let name_length = u16::try_from(name.len()).expect("entra en 16 bits");
        archive.extend(0x0403_4b50_u32.to_le_bytes());
        archive.extend([0; 4]);
        archive.extend(method.to_le_bytes());
        archive.extend([0; 8]);
        archive.extend(size.to_le_bytes());
        archive.extend(size.to_le_bytes());
        archive.extend(name_length.to_le_bytes());
        archive.extend([0; 2]);
        archive.extend(name.as_bytes());
        archive.extend(&data);
        central.extend(0x0201_4b50_u32.to_le_bytes());
        central.extend([0; 6]);
        central.extend(method.to_le_bytes());
        central.extend([0; 8]);
        central.extend(size.to_le_bytes());
        central.extend(size.to_le_bytes());
        central.extend(name_length.to_le_bytes());
        central.extend([0; 12]);
        central.extend(offset.to_le_bytes());
        central.extend(name.as_bytes());
    }
    let start = u32::try_from(archive.len()).expect("entra en 32 bits");
    let count = u16::try_from(files.len()).expect("entra en 16 bits");
    let central_size = u32::try_from(central.len()).expect("entra en 32 bits");
    archive.extend(central);
    archive.extend(0x0605_4b50_u32.to_le_bytes());
    archive.extend([0; 4]);
    archive.extend(count.to_le_bytes());
    archive.extend(count.to_le_bytes());
    archive.extend(central_size.to_le_bytes());
    archive.extend(start.to_le_bytes());
    archive.extend([0; 2]);
    archive
}

#[test]
fn the_test_archive_is_read_back_entry_by_entry() {
    let tree = TempTree::new("osv-zip");
    let path: PathBuf = tree.path().join("all.zip");
    fs::write(&path, zip(&[("a.json", "hola", true), ("b.json", "chau", false)])).expect("se escribe el zip");
    let mut archive = super::zip::Archive::open(fs::File::open(&path).expect("se abre el zip"));
    let entries = archive.entries().expect("el índice se lee");
    let bodies: Vec<Vec<u8>> = entries
        .into_iter()
        .map(|entry| archive.read(entry).expect("la entrada se lee"))
        .collect();
    assert_eq!(bodies, [b"hola".to_vec(), b"chau".to_vec()]);
    fs::write(&path, b"no es un zip").expect("se escribe");
    let mut broken = super::zip::Archive::open(fs::File::open(&path).expect("se abre"));
    assert!(broken.entries().is_err());
}
