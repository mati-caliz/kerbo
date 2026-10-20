mod lockfiles;
mod offline;
mod online;
mod record;
mod responses;
mod transport;
mod version;
mod zip;

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::ValueEnum;

use crate::cache::cache_root;
use crate::config::Config;
use crate::i18n::{Advisory, Message, Notice, OsvProblem, Problem};
use crate::rules::{Diagnostic, FileIdentity, RuleId};
use crate::scan::identities;
use lockfiles::{Dependency, Format};
use record::Record;
use responses::Responses;
pub use transport::{Curl, Transport};

const OSV_DIRECTORY: &str = "osv-v1";
const RESPONSES_DIRECTORY: &str = "responses";
const DATABASE_DIRECTORY: &str = "db";
const FOUND_ONCE: usize = 1;
const FIRST_COLUMN: usize = 1;
const NOTHING_ALLOWED: usize = 0;
const PYPI_SEPARATORS: &[char] = &['-', '_', '.'];
const PYPI_SEPARATOR: &str = "-";
const CVE_PREFIX: &str = "CVE-";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum)]
pub enum Ecosystem {
    #[value(name = "crates.io")]
    CratesIo,
    #[value(name = "npm")]
    Npm,
    #[value(name = "Go")]
    Go,
    #[value(name = "PyPI")]
    PyPi,
    #[value(name = "Maven")]
    Maven,
}

impl Ecosystem {
    pub const ALL: [Self; 5] = [Self::CratesIo, Self::Npm, Self::Go, Self::PyPi, Self::Maven];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CratesIo => "crates.io",
            Self::Npm => "npm",
            Self::Go => "Go",
            Self::PyPi => "PyPI",
            Self::Maven => "Maven",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OsvMode {
    #[default]
    Online,
    Offline,
}

#[derive(Debug)]
pub struct AuditRequest<'request> {
    pub roots: &'request [PathBuf],
    pub config: &'request Config,
    pub mode: OsvMode,
    pub transport: &'request dyn Transport,
    pub directory: Option<PathBuf>,
}

pub fn normalized_name(ecosystem: Ecosystem, name: &str) -> String {
    match ecosystem {
        Ecosystem::PyPi => name
            .to_lowercase()
            .split(PYPI_SEPARATORS)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(PYPI_SEPARATOR),
        Ecosystem::CratesIo | Ecosystem::Npm | Ecosystem::Go | Ecosystem::Maven => name.to_owned(),
    }
}

pub fn default_directory() -> Option<PathBuf> {
    cache_root().map(|root| root.join(OSV_DIRECTORY))
}

pub fn audit(request: &AuditRequest<'_>) -> Result<Vec<Diagnostic>, Problem> {
    let lockfiles = read_lockfiles(request.roots, request.config);
    let dependencies: Vec<&Dependency> = lockfiles.iter().flat_map(|(_, found)| found).collect();
    if dependencies.is_empty() {
        return Ok(Vec::new());
    }
    let directory = request.directory.clone().or_else(default_directory);
    let found = match (request.mode, directory) {
        (OsvMode::Offline, Some(directory)) => {
            offline::lookup(&dependencies, &directory.join(DATABASE_DIRECTORY))
        }
        (OsvMode::Offline, None) => Err(OsvProblem::DatabaseMissing(DATABASE_DIRECTORY.to_owned())),
        (OsvMode::Online, directory) => {
            let mut responses = Responses::open(
                directory.unwrap_or_default().join(RESPONSES_DIRECTORY),
                seconds_since_epoch(),
            );
            let found = online::lookup(&dependencies, request.transport, &mut responses);
            responses.persist();
            found
        }
    }
    .map_err(Problem::Osv)?;
    Ok(lockfiles
        .iter()
        .flat_map(|(identity, declared)| {
            declared.iter().flat_map(|dependency| {
                let records = found
                    .get(&online::query_key(dependency))
                    .cloned()
                    .unwrap_or_default();
                diagnostics(identity, dependency, &records)
            })
        })
        .collect())
}

pub fn download(
    ecosystems: &[Ecosystem],
    transport: &dyn Transport,
    directory: Option<PathBuf>,
) -> Result<Vec<Notice>, Problem> {
    let database = directory
        .or_else(default_directory)
        .ok_or_else(|| Problem::Osv(OsvProblem::DatabaseMissing(DATABASE_DIRECTORY.to_owned())))?
        .join(DATABASE_DIRECTORY);
    let wanted = if ecosystems.is_empty() {
        &Ecosystem::ALL[..]
    } else {
        ecosystems
    };
    wanted
        .iter()
        .map(|&ecosystem| {
            let (advisories, path) =
                offline::download(ecosystem, transport, &database).map_err(Problem::Osv)?;
            Ok(Notice::OsvDatabaseSaved {
                ecosystem: ecosystem.as_str().to_owned(),
                advisories,
                path: path.display().to_string(),
            })
        })
        .collect()
}

fn seconds_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn read_lockfiles(roots: &[PathBuf], config: &Config) -> Vec<(FileIdentity, Vec<Dependency>)> {
    let (files, _) = identities(roots, config);
    let mut lockfiles: Vec<(FileIdentity, Format, Vec<Dependency>)> = files
        .into_iter()
        .filter(|identity| config.reports(Path::new(&identity.relative), RuleId::VulnerableDependency))
        .filter_map(|identity| {
            let format = Format::of(identity.absolute.file_name()?.to_str()?)?;
            let text = fs::read_to_string(&identity.absolute).ok()?;
            let dependencies = format.parse(&identity.absolute, &text);
            Some((identity, format, dependencies))
        })
        .collect();
    let declared: HashSet<(Option<PathBuf>, String, String)> = lockfiles
        .iter()
        .filter(|(_, format, _)| *format == Format::GoMod)
        .flat_map(|(identity, _, dependencies)| {
            dependencies.iter().map(|dependency| {
                (
                    parent(identity),
                    dependency.name.clone(),
                    dependency.version.clone(),
                )
            })
        })
        .collect();
    for (identity, format, dependencies) in &mut lockfiles {
        if *format == Format::GoSum {
            let directory = parent(identity);
            dependencies.retain(|dependency| {
                !declared.contains(&(
                    directory.clone(),
                    dependency.name.clone(),
                    dependency.version.clone(),
                ))
            });
        }
    }
    lockfiles
        .into_iter()
        .map(|(identity, _, dependencies)| (identity, dependencies))
        .collect()
}

fn parent(identity: &FileIdentity) -> Option<PathBuf> {
    identity.absolute.parent().map(Path::to_path_buf)
}

fn diagnostics(identity: &FileIdentity, dependency: &Dependency, records: &[Record]) -> Vec<Diagnostic> {
    groups(records)
        .into_iter()
        .map(|group| {
            let ids: Vec<&str> = group.iter().map(|record| record.id.as_str()).collect();
            let primary = ids.first().copied().unwrap_or_default().to_owned();
            let aliases: BTreeSet<String> = group
                .iter()
                .flat_map(|record| record.aliases.iter().chain(std::iter::once(&record.id)))
                .filter(|alias| **alias != primary)
                .cloned()
                .collect();
            let lead = group.first().copied();
            let advisory = Advisory {
                package: dependency.name.clone(),
                version: dependency.version.clone(),
                id: primary,
                aliases: ordered_aliases(aliases),
                summary: lead.map(Record::summary).unwrap_or_default(),
                fixed: group.iter().find_map(|record| record.fixed_version(dependency)),
            };
            diagnostic(identity, dependency, advisory)
        })
        .collect()
}

fn ordered_aliases(aliases: BTreeSet<String>) -> Vec<String> {
    let (cves, others): (Vec<String>, Vec<String>) = aliases
        .into_iter()
        .partition(|alias| alias.starts_with(CVE_PREFIX));
    cves.into_iter().chain(others).collect()
}

fn groups(records: &[Record]) -> Vec<Vec<&Record>> {
    let mut sorted: Vec<&Record> = records.iter().collect();
    sorted.sort_by(|left, right| left.id.cmp(&right.id));
    sorted.dedup_by(|left, right| left.id == right.id);
    let mut groups: Vec<(BTreeSet<&str>, Vec<&Record>)> = Vec::new();
    for record in sorted {
        let names: BTreeSet<&str> = std::iter::once(record.id.as_str())
            .chain(record.aliases.iter().map(String::as_str))
            .collect();
        match groups.iter_mut().find(|(known, _)| !known.is_disjoint(&names)) {
            Some((known, members)) => {
                known.extend(names);
                members.push(record);
            }
            None => groups.push((names, vec![record])),
        }
    }
    groups.into_iter().map(|(_, members)| members).collect()
}

fn diagnostic(identity: &FileIdentity, dependency: &Dependency, advisory: Advisory) -> Diagnostic {
    let fingerprint = format!(
        "{}:{}:{}@{}:{}",
        RuleId::VulnerableDependency.as_str(),
        identity.relative,
        dependency.name,
        dependency.version,
        advisory.id
    );
    Diagnostic {
        path: identity.display.clone(),
        line: dependency.line,
        column: FIRST_COLUMN,
        end_line: dependency.line,
        rule: RuleId::VulnerableDependency,
        message: Message::VulnerableDependency(Box::new(advisory)),
        value: FOUND_ONCE,
        limit: NOTHING_ALLOWED,
        fingerprint,
        relative_path: identity.relative.clone(),
        absolute_path: identity.absolute.clone(),
        fix: None,
    }
}

#[cfg(test)]
mod tests;
