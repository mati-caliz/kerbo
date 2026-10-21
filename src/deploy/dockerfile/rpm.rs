use std::collections::{HashMap, HashSet};

use super::image::BaseImage;
use super::parser::Keyword;
use super::shell::{Command, Script};
use super::stages::environment_pairs;
use super::{Findings, Step, has_caching_mount};
use crate::rules::DockerfileRule;

const YUM: &str = "yum";
const ZYPPER: &str = "zypper";
const DNF_COMMANDS: &[&str] = &["dnf", "microdnf"];
const YUM_INSTALLS: &[&str] = &["install", "groupinstall", "localinstall"];
const ASSUME_YES: &[&str] = &["y", "assumeyes"];
const ZYPPER_CHANGES: &[&str] = &["install", "in", "remove", "rm", "source-install", "si", "patch"];
const ZYPPER_NON_INTERACTIVE: &[&str] = &["non-interactive", "n", "no-confirm", "y"];
const ZYPPER_DIST_UPGRADE: &[&str] = &["dist-upgrade", "dup"];
const ZYPPER_INSTALLS: &[&str] = &["install", "in"];
const ZYPPER_CLEANS: &[&str] = &["clean", "cc"];
const ZYPPER_CACHE: &str = "/var/cache/zypp";
const ZYPPER_PINS: &[&str] = &["=", ">", "<"];
const DNF_INSTALLS: &[&str] = &[
    "install",
    "in",
    "upgrade",
    "up",
    "upgrade-minimal",
    "up-min",
    "reinstall",
    "rei",
];
const DNF_CACHES: &[&str] = &["/var/cache/libdnf5", ".cache/libdnf5"];
const DNF_CACHE_REMOVAL: &[&str] = &["-rf", "/var/cache/libdnf5*"];
const RPM_SUFFIX: &str = ".rpm";
const VERSION_SEPARATOR: char = '-';
const MODULE_STREAM: &str = ":";
const VERSION_SYMBOLS: &[char] = &['.', '~', '^', '_', ':', '+'];
const VARIABLE_MARK: &str = "$";

pub(super) fn check(steps: &[Step], findings: &mut Findings) {
    let mut variables = Variables::default();
    for step in steps {
        let instruction = &step.instruction;
        variables.update(step);
        let Some(script) = &step.script else {
            continue;
        };
        let checks = [
            (
                DockerfileRule::Dl3030,
                script.any(|command| yum_install(command) && !command.has_any_flag(ASSUME_YES)),
            ),
            (
                DockerfileRule::Dl3033,
                !versions_pinned(script, &YUM_PACKAGES, &variables),
            ),
            (
                DockerfileRule::Dl3034,
                script.any(|command| {
                    command.has_any_argument(ZYPPER, ZYPPER_CHANGES)
                        && !command.has_any_flag(ZYPPER_NON_INTERACTIVE)
                }),
            ),
            (
                DockerfileRule::Dl3035,
                script.any(|command| command.has_any_argument(ZYPPER, ZYPPER_DIST_UPGRADE)),
            ),
            (
                DockerfileRule::Dl3036,
                !has_caching_mount(instruction, ZYPPER_CACHE)
                    && cleans_too_late(script, zypper_install, zypper_clean),
            ),
            (
                DockerfileRule::Dl3037,
                script.commands.iter().any(zypper_unpinned),
            ),
            (DockerfileRule::Dl3038, script.any(dnf_install_without_yes)),
            (
                DockerfileRule::Dl3040,
                !DNF_CACHES
                    .iter()
                    .any(|cache| has_caching_mount(instruction, cache))
                    && DNF_COMMANDS.iter().any(|name| {
                        cleans_too_late(
                            script,
                            |command| dnf_change(command, name),
                            |command| dnf_clean(command, name),
                        )
                    }),
            ),
            (
                DockerfileRule::Dl3041,
                !versions_pinned(script, &DNF_PACKAGES, &variables),
            ),
        ];
        for (rule, failed) in checks {
            if failed {
                findings.report(rule, instruction.line, "");
            }
        }
    }
}

fn yum_install(command: &Command) -> bool {
    command.has_any_argument(YUM, YUM_INSTALLS)
}

fn dnf_install_without_yes(command: &Command) -> bool {
    DNF_COMMANDS
        .iter()
        .any(|name| command.has_any_argument(name, YUM_INSTALLS))
        && !command.has_any_flag(ASSUME_YES)
}

fn zypper_install(command: &Command) -> bool {
    command.has_any_argument(ZYPPER, ZYPPER_INSTALLS)
}

fn zypper_clean(command: &Command) -> bool {
    command.has_any_argument(ZYPPER, ZYPPER_CLEANS)
}

fn dnf_change(command: &Command, name: &str) -> bool {
    command.has_any_argument(name, DNF_INSTALLS)
}

fn dnf_clean(command: &Command, name: &str) -> bool {
    command.has_any_argument(name, &["clean", "all"]) || command.has_any_argument("rm", DNF_CACHE_REMOVAL)
}

fn cleans_too_late(
    script: &Script,
    installs: impl Fn(&Command) -> bool,
    cleans: impl Fn(&Command) -> bool,
) -> bool {
    let Some(install) = script.position(installs) else {
        return false;
    };
    script.position(cleans).is_none_or(|clean| install >= clean)
}

fn zypper_unpinned(command: &Command) -> bool {
    zypper_install(command)
        && command
            .arguments_without_flags()
            .into_iter()
            .filter(|argument| !ZYPPER_INSTALLS.contains(argument))
            .any(|package| {
                !ZYPPER_PINS.iter().any(|pin| package.contains(pin)) && !package.ends_with(RPM_SUFFIX)
            })
}

struct PackageManager {
    names: &'static [&'static str],
    ignored_subcommands: &'static [&'static str],
}

const YUM_PACKAGES: PackageManager = PackageManager {
    names: &[YUM],
    ignored_subcommands: &[],
};
const DNF_PACKAGES: PackageManager = PackageManager {
    names: DNF_COMMANDS,
    ignored_subcommands: &["group"],
};
const MODULE_SUBCOMMAND: &str = "module";
const INSTALL_SUBCOMMAND: &str = "install";

fn versions_pinned(script: &Script, manager: &PackageManager, variables: &Variables) -> bool {
    script.commands.iter().all(|command| {
        let has = |subcommands: &[&str]| {
            manager
                .names
                .iter()
                .any(|name| command.has_any_argument(name, subcommands))
        };
        let packages = installed(command, manager.names);
        if has(&[MODULE_SUBCOMMAND]) {
            packages.iter().all(|module| module.contains(MODULE_STREAM))
        } else if has(manager.ignored_subcommands) {
            true
        } else {
            packages.iter().all(|package| package_pinned(package, variables))
        }
    })
}

fn installed<'command>(command: &'command Command, names: &[&str]) -> Vec<&'command str> {
    if !names
        .iter()
        .any(|name| command.has_any_argument(name, &[INSTALL_SUBCOMMAND]))
    {
        return Vec::new();
    }
    command
        .arguments_without_flags()
        .into_iter()
        .filter(|argument| *argument != INSTALL_SUBCOMMAND && *argument != MODULE_SUBCOMMAND)
        .collect()
}

fn package_pinned(package: &str, variables: &Variables) -> bool {
    let parts: Vec<&str> = package.split(VERSION_SEPARATOR).collect();
    if parts.len() <= 1 {
        return false;
    }
    if package.ends_with(RPM_SUFFIX) {
        return true;
    }
    if package.contains(VARIABLE_MARK) {
        return variables.defined_in(package);
    }
    let version = parts.get(1..).unwrap_or_default();
    version.iter().all(|part| {
        part.chars()
            .all(|character| character.is_ascii_alphanumeric() || VERSION_SYMBOLS.contains(&character))
    }) && version
        .iter()
        .any(|part| part.starts_with(|first: char| first.is_ascii_digit()))
}

#[derive(Debug, Default)]
struct Variables {
    stage: usize,
    aliases: HashMap<usize, String>,
    environment: HashMap<usize, HashSet<String>>,
    arguments: HashSet<String>,
}

impl Variables {
    fn update(&mut self, step: &Step) {
        let instruction = &step.instruction;
        match instruction.keyword {
            Keyword::From => self.new_stage(&BaseImage::parse(instruction)),
            Keyword::Env => {
                let names = environment_pairs(&instruction.arguments)
                    .into_iter()
                    .map(|(name, _)| name);
                self.environment.entry(self.stage).or_default().extend(names);
            }
            Keyword::Arg => {
                let name = instruction.arguments.split(['=', ' ']).next().unwrap_or_default();
                if !name.is_empty() {
                    self.arguments.insert(name.to_owned());
                }
            }
            Keyword::Run
            | Keyword::Copy
            | Keyword::Add
            | Keyword::User
            | Keyword::Workdir
            | Keyword::Cmd
            | Keyword::Entrypoint
            | Keyword::Healthcheck
            | Keyword::Shell
            | Keyword::Maintainer
            | Keyword::Expose
            | Keyword::Onbuild
            | Keyword::Label
            | Keyword::Other => {}
        }
    }

    fn new_stage(&mut self, image: &BaseImage) {
        let next = self.stage.saturating_add(1);
        let parent = self
            .aliases
            .iter()
            .filter(|(_, alias)| **alias == image.name)
            .map(|(index, _)| *index)
            .min();
        let inherited = parent
            .and_then(|index| self.environment.get(&index))
            .cloned()
            .unwrap_or_default();
        self.environment.insert(next, inherited);
        self.aliases.insert(next, image.alias.clone().unwrap_or_default());
        self.stage = next;
    }

    fn defined_in(&self, package: &str) -> bool {
        let reference = |name: &String| package.contains(&format!("${{{name}}}"));
        self.environment
            .get(&self.stage)
            .is_some_and(|names| names.iter().any(reference))
            || self.arguments.iter().any(reference)
    }
}
