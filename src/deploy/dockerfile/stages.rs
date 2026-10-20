mod runs;

use std::collections::{BTreeMap, HashMap, HashSet};

use super::image::BaseImage;
use super::instructions::is_windows_absolute;
use super::parser::{Instruction, Keyword};
use super::{Findings, Step, unquoted};
use crate::rules::DockerfileRule;

const SCRATCH: &str = "scratch";
const VARIABLE_PREFIX: &str = "$";
const ROOT_USERS: &[&str] = &["root", "0"];
const ROOT_PREFIXES: &[&str] = &["root:", "0:"];
const FROM_FLAG: &str = "from";
const IMAGE_REFERENCE: char = ':';
const ENV_SEPARATOR: char = '=';

#[derive(Debug, Default)]
struct Stage {
    alias: Option<String>,
    healthchecks: usize,
    commands: usize,
    entrypoints: usize,
}

pub(super) fn check(steps: &[Step], findings: &mut Findings) {
    let plain: Vec<&Instruction> = steps
        .iter()
        .map(|step| &step.instruction)
        .filter(|instruction| !instruction.onbuild)
        .collect();
    structure(&plain, findings);
    last_root_users(&plain, findings);
    relative_copies(&plain, findings);
    runs::check(steps, findings);
}

#[derive(Debug, Default)]
struct Stages {
    current: Stage,
    aliases: HashSet<String>,
    count: usize,
}

impl Stages {
    fn enter(&mut self, image: BaseImage, line: usize, findings: &mut Findings) {
        untagged(&image, &self.aliases, line, findings);
        if let Some(alias) = &image.alias
            && !self.aliases.insert(alias.clone())
        {
            findings.report(DockerfileRule::Dl3024, line, alias.clone());
        }
        self.count = self.count.saturating_add(1);
        self.current = Stage {
            alias: image.alias,
            ..Stage::default()
        };
    }
}

fn structure(instructions: &[&Instruction], findings: &mut Findings) {
    let mut stages = Stages::default();
    for instruction in instructions {
        let line = instruction.line;
        let stage = &mut stages.current;
        match instruction.keyword {
            Keyword::From => stages.enter(BaseImage::parse(instruction), line, findings),
            Keyword::Healthcheck => repeated(&mut stage.healthchecks, DockerfileRule::Dl3012, line, findings),
            Keyword::Cmd => repeated(&mut stage.commands, DockerfileRule::Dl4003, line, findings),
            Keyword::Entrypoint => repeated(&mut stage.entrypoints, DockerfileRule::Dl4004, line, findings),
            Keyword::Copy => copy_source(instruction, &stages, findings),
            Keyword::Run
            | Keyword::Add
            | Keyword::User
            | Keyword::Workdir
            | Keyword::Env
            | Keyword::Arg
            | Keyword::Shell
            | Keyword::Maintainer
            | Keyword::Other => {}
        }
    }
}

fn repeated(count: &mut usize, rule: DockerfileRule, line: usize, findings: &mut Findings) {
    if *count > 0 {
        findings.report(rule, line, String::new());
    }
    *count = count.saturating_add(1);
}

fn untagged(image: &BaseImage, aliases: &HashSet<String>, line: usize, findings: &mut Findings) {
    let exempt = image.name == SCRATCH
        || image.digest.is_some()
        || image.tag.is_some()
        || image.name.starts_with(VARIABLE_PREFIX)
        || aliases.contains(&image.name);
    if !exempt {
        findings.report(DockerfileRule::Dl3006, line, image.name.clone());
    }
}

fn copy_source(instruction: &Instruction, stages: &Stages, findings: &mut Findings) {
    let Some(source) = instruction.flag(FROM_FLAG) else {
        return;
    };
    let line = instruction.line;
    if stages.current.alias.as_deref() == Some(source) {
        findings.report(DockerfileRule::Dl3023, line, source);
    }
    let known = unquoted(source).contains(IMAGE_REFERENCE)
        || stages.aliases.contains(source)
        || source.parse::<usize>().is_ok_and(|index| index < stages.count);
    if !known {
        findings.report(DockerfileRule::Dl3022, line, source);
    }
}

fn last_root_users(instructions: &[&Instruction], findings: &mut Findings) {
    let mut root_users: BTreeMap<usize, (usize, String)> = BTreeMap::new();
    let mut stage_line = None;
    for instruction in instructions {
        match instruction.keyword {
            Keyword::From => stage_line = Some(instruction.line),
            Keyword::User => {
                let Some(stage) = stage_line else {
                    continue;
                };
                let user = instruction.arguments.trim();
                let is_root =
                    ROOT_USERS.contains(&user) || ROOT_PREFIXES.iter().any(|prefix| user.starts_with(prefix));
                if is_root {
                    root_users.insert(stage, (instruction.line, user.to_owned()));
                } else {
                    root_users.remove(&stage);
                }
            }
            Keyword::Run
            | Keyword::Copy
            | Keyword::Add
            | Keyword::Workdir
            | Keyword::Env
            | Keyword::Arg
            | Keyword::Cmd
            | Keyword::Entrypoint
            | Keyword::Healthcheck
            | Keyword::Shell
            | Keyword::Maintainer
            | Keyword::Other => {}
        }
    }
    for (line, user) in root_users.into_values() {
        findings.report(DockerfileRule::Dl3002, line, user);
    }
}

#[derive(Debug, Default)]
pub(super) struct StageFlags {
    current: Option<String>,
    set: HashMap<Option<String>, bool>,
}

impl StageFlags {
    pub(super) fn enter(&mut self, image: &BaseImage) {
        let inherited = self
            .set
            .get(&Some(image.name.clone()))
            .copied()
            .unwrap_or_default();
        let current = Some(image.stage_name().to_owned());
        self.set.insert(current.clone(), inherited);
        self.current = current;
    }

    pub(super) fn mark(&mut self) {
        self.set.insert(self.current.clone(), true);
    }

    pub(super) fn is_set(&self) -> bool {
        self.set.get(&self.current).copied().unwrap_or_default()
    }
}

fn relative_copies(instructions: &[&Instruction], findings: &mut Findings) {
    let mut workdirs = StageFlags::default();
    for instruction in instructions {
        match instruction.keyword {
            Keyword::From => workdirs.enter(&BaseImage::parse(instruction)),
            Keyword::Workdir => workdirs.mark(),
            Keyword::Copy if instruction.heredoc.is_none() => {
                let target = copy_target(instruction);
                let destination = unquoted(&target);
                let absolute = destination.starts_with('/')
                    || is_windows_absolute(destination)
                    || destination.starts_with(VARIABLE_PREFIX);
                if !workdirs.is_set() && !absolute {
                    findings.report(DockerfileRule::Dl3045, instruction.line, destination);
                }
            }
            Keyword::Run
            | Keyword::Copy
            | Keyword::Add
            | Keyword::User
            | Keyword::Env
            | Keyword::Arg
            | Keyword::Cmd
            | Keyword::Entrypoint
            | Keyword::Healthcheck
            | Keyword::Shell
            | Keyword::Maintainer
            | Keyword::Other => {}
        }
    }
}

fn copy_target(instruction: &Instruction) -> String {
    instruction
        .exec_form()
        .and_then(|paths| paths.last().cloned())
        .or_else(|| instruction.arguments.split_whitespace().last().map(str::to_owned))
        .unwrap_or_default()
}

pub(super) fn environment_pairs(text: &str) -> Vec<(String, String)> {
    let first = text.split_whitespace().next().unwrap_or_default();
    if !first.contains(ENV_SEPARATOR) {
        let value = text.get(first.len()..).unwrap_or_default().trim();
        return vec![(first.to_owned(), value.to_owned())];
    }
    super::shell::words(text)
        .into_iter()
        .filter_map(|word| {
            word.split_once(ENV_SEPARATOR)
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
        })
        .collect()
}
