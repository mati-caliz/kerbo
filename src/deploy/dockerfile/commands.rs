use std::collections::{BTreeSet, HashSet};

use super::image::BaseImage;
use super::instructions::paths;
use super::parser::{Instruction, Keyword};
use super::shell::{Command, Script};
use super::{Findings, Step, has_caching_mount};
use crate::rules::DockerfileRule;

const POINTLESS_COMMANDS: &[&str] = &[
    "free", "kill", "mount", "ps", "service", "shutdown", "ssh", "top", "vim",
];
const GEM: &str = "gem";
const GEM_INSTALLS: &[&str] = &["install", "i"];
const GEM_VERSION_FLAGS: &[&str] = &["-v", "--version"];
const GEM_VERSION_PREFIX: &str = "--version=";
const END_OF_OPTIONS: &str = "--";
const OPTION_PREFIX: &str = "-";
const GEM_VERSION_SEPARATOR: &str = ":";
const USERADD: &str = "useradd";
const NO_LOG_INIT: &[&str] = &["l", "no-log-init"];
const UID_FLAGS: &[&str] = &["u", "uid"];
const UID_SHORT_FLAG: &str = "u";
const LONG_UID_DIGITS: usize = 5;
const LN: &str = "ln";
const DEFAULT_SHELL: &str = "/bin/sh";
const FROM_FLAG: &str = "from";
const ARCHIVE_EXTENSIONS: &[&str] = &[
    ".tar", ".Z", ".bz2", ".gz", ".lz", ".lzma", ".tZ", ".tb2", ".tbz", ".tbz2", ".tgz", ".tlz", ".tpz",
    ".txz", ".xz",
];
const TAR: &str = "tar";
const TAR_EXTRACTS: &[&str] = &["--extract", "--get"];
const SHORT_EXTRACT: char = 'x';
const UNPACKERS: &[&str] = &[
    "unzip",
    "gunzip",
    "bunzip2",
    "unlzma",
    "unxz",
    "zgz",
    "uncompress",
    "zcat",
    "gzcat",
];
const PATH_SEPARATORS: &[char] = &['/', '\\'];
const QUOTES: &[char] = &['"', '\''];
const YARN: &str = "yarn";
const YARN_CACHE: &str = ".cache/yarn";

pub(super) fn check(steps: &[Step], findings: &mut Findings) {
    for step in steps {
        let Some(script) = &step.script else {
            continue;
        };
        let checks = [
            (
                DockerfileRule::Dl3001,
                script.any(|command| POINTLESS_COMMANDS.contains(&command.name.as_str())),
            ),
            (
                DockerfileRule::Dl3028,
                !gems(script).iter().all(|gem| gem.contains(GEM_VERSION_SEPARATOR)),
            ),
            (DockerfileRule::Dl3046, script.any(large_uid_without_l)),
            (
                DockerfileRule::Dl4005,
                script.any(|command| command.has_any_argument(LN, &[DEFAULT_SHELL])),
            ),
        ];
        for (rule, failed) in checks {
            if failed {
                findings.report(rule, step.instruction.line, "");
            }
        }
    }
    extracted_archives(steps, findings);
    yarn_caches(steps, findings);
}

fn gems(script: &Script) -> Vec<&str> {
    script
        .commands
        .iter()
        .filter(|command| {
            command.has_any_argument(GEM, GEM_INSTALLS)
                && !command.has_any_argument(GEM, GEM_VERSION_FLAGS)
                && !command
                    .arguments
                    .iter()
                    .any(|argument| argument.starts_with(GEM_VERSION_PREFIX))
        })
        .flat_map(|command| {
            let mut kept = Vec::new();
            let mut arguments = command
                .arguments
                .iter()
                .take_while(|argument| *argument != END_OF_OPTIONS);
            while let Some(argument) = arguments.next() {
                if argument.starts_with(OPTION_PREFIX) {
                    arguments.next();
                } else if !GEM_INSTALLS.contains(&argument.as_str()) {
                    kept.push(argument.as_str());
                }
            }
            kept
        })
        .collect()
}

fn large_uid_without_l(command: &Command) -> bool {
    command.is(USERADD)
        && !command.has_any_flag(NO_LOG_INIT)
        && command.has_any_flag(UID_FLAGS)
        && command
            .flag_arguments(UID_SHORT_FLAG)
            .iter()
            .any(|uid| uid.chars().count() > LONG_UID_DIGITS)
}

fn basename(path: &str) -> &str {
    let unquoted = path.trim_matches(QUOTES);
    unquoted.rsplit(PATH_SEPARATORS).next().unwrap_or(unquoted)
}

fn is_archive(path: &str) -> bool {
    let unquoted = path.trim_matches(QUOTES);
    ARCHIVE_EXTENSIONS
        .iter()
        .any(|extension| unquoted.ends_with(extension))
}

fn extracted_archives(steps: &[Step], findings: &mut Findings) {
    let mut archives: BTreeSet<(usize, String)> = BTreeSet::new();
    let mut extracted: BTreeSet<(usize, String)> = BTreeSet::new();
    for step in steps.iter().filter(|step| !step.instruction.onbuild) {
        let instruction = &step.instruction;
        if instruction.keyword == Keyword::From {
            archives.clear();
            extracted.clear();
        } else if instruction.keyword == Keyword::Copy {
            archives.extend(copied_archives(instruction));
        }
        if let Some(script) = &step.script {
            extracted.extend(newly_extracted(script, &archives));
        }
    }
    report_archives(&extracted, findings);
}

fn copied_archives(instruction: &Instruction) -> Vec<(usize, String)> {
    if instruction.flag(FROM_FLAG).is_some() || instruction.heredoc.is_some() {
        return Vec::new();
    }
    let paths = paths(instruction);
    let Some((target, sources)) = paths.split_last() else {
        return Vec::new();
    };
    if is_archive(target) {
        return vec![(instruction.line, basename(target).to_owned())];
    }
    sources
        .iter()
        .map(|source| basename(source))
        .filter(|name| is_archive(name))
        .map(|name| (instruction.line, name.to_owned()))
        .collect()
}

fn newly_extracted(script: &Script, archives: &BTreeSet<(usize, String)>) -> Vec<(usize, String)> {
    let extracting: Vec<&Command> = script
        .commands
        .iter()
        .filter(|command| extracts(command))
        .collect();
    archives
        .iter()
        .filter(|(_, name)| {
            extracting.iter().any(|command| {
                command
                    .arguments_without_flags()
                    .iter()
                    .any(|argument| basename(argument) == name)
            })
        })
        .cloned()
        .collect()
}

fn report_archives(extracted: &BTreeSet<(usize, String)>, findings: &mut Findings) {
    for (line, name) in extracted {
        findings.report(DockerfileRule::Dl3010, *line, name.clone());
    }
}

fn extracts(command: &Command) -> bool {
    let tar = command.is(TAR)
        && command.arguments.iter().any(|argument| {
            TAR_EXTRACTS.contains(&argument.as_str())
                || (argument.starts_with(OPTION_PREFIX) && argument.contains(SHORT_EXTRACT))
        });
    tar || UNPACKERS.contains(&command.name.as_str())
}

fn yarn_caches(steps: &[Step], findings: &mut Findings) {
    let mut current: Option<BaseImage> = None;
    let mut used_as_base: HashSet<String> = HashSet::new();
    let mut failing: Vec<(usize, Option<BaseImage>)> = Vec::new();
    for step in steps.iter().filter(|step| !step.instruction.onbuild) {
        let instruction = &step.instruction;
        if instruction.keyword == Keyword::From {
            let image = BaseImage::parse(instruction);
            used_as_base.insert(image.name.clone());
            current = Some(image);
            continue;
        }
        let Some(script) = &step.script else {
            continue;
        };
        let install = script.position(|command| command.has_any_argument(YARN, &["install"]));
        let clean = script.position(|command| command.has_any_argument(YARN, &["cache", "clean"]));
        let missing_clean =
            install.is_some() && clean.is_none() && !has_caching_mount(instruction, YARN_CACHE);
        let clean_first = matches!((clean, install), (Some(clean), Some(install)) if clean < install);
        if missing_clean || clean_first {
            failing.push((instruction.line, current.clone()));
        }
    }
    for (line, stage) in failing {
        let last_stage = stage == current;
        let reused = stage
            .as_ref()
            .and_then(|stage| stage.alias.as_ref())
            .is_some_and(|alias| used_as_base.contains(alias));
        if last_stage || reused {
            findings.report(DockerfileRule::Dl3060, line, "");
        }
    }
}
