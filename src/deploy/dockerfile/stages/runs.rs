use std::collections::HashSet;

use super::super::image::BaseImage;
use super::super::packages::is_pip_install;
use super::super::parser::{Flag, Keyword};
use super::super::shell::{Command, Script};
use super::super::{Findings, Step, has_caching_mount};
use super::{StageFlags, environment_pairs};
use crate::rules::DockerfileRule;

const SINGLE_COMMAND: usize = 1;
const PIPEFAIL_SHELLS: &[&str] = &["/bin/bash", "/bin/zsh", "/bin/ash", "bash", "zsh", "ash"];
const NON_POSIX_SHELLS: &[&str] = &["pwsh", "powershell", "cmd"];
const PIPEFAIL: &str = "pipefail";
const DOWNLOADERS: &[&str] = &["curl", "wget"];
const APT_LISTS: &str = "/var/lib/apt/lists";
const APT_DIRECTORY: &str = "/var/lib/apt";
const APT_CACHE: &str = "/var/cache/apt";
const APT_LISTS_CLEANUP: &[&str] = &["-rf", "/var/lib/apt/lists/*"];
const PACKAGE_MANAGERS: &[&str] = &["apt", "apt-get", "aptitude"];
const UPDATE: &str = "update";
const PIP_CACHE: &str = ".cache/pip";
const PIP_NO_CACHE_VARIABLE: &str = "PIP_NO_CACHE_DIR";
const PIP_NO_CACHE_ASSIGNMENT: &str = "PIP_NO_CACHE_DIR=";
const NO_CACHE_DIR: &str = "--no-cache-dir";
const PIP_WRAPPERS: &[&str] = &["pipx", "pipenv"];
const PYTHON_PREFIX: &str = "python";
const MODULE_FLAG: &str = "-m";
const TRUTHY: &[&str] = &["1", "true", "True", "TRUE", "on", "On", "ON", "yes", "Yes", "YES"];

#[derive(Debug, Default)]
struct RunState<'steps> {
    previous_run: Option<(&'steps [Flag], usize)>,
    pipefail: bool,
    downloaders: HashSet<&'static str>,
    current_image: Option<BaseImage>,
    parent_images: HashSet<String>,
    uncleaned_lists: Vec<(usize, Option<BaseImage>)>,
    pip_cache_disabled: StageFlags,
}

pub(super) fn check(steps: &[Step], findings: &mut Findings) {
    let mut state = RunState::default();
    for step in steps {
        let instruction = &step.instruction;
        let script = step.script.as_ref().filter(|_| !instruction.onbuild);
        if script.is_some() {
            consecutive_runs(&mut state, step, findings);
        } else {
            state.previous_run = None;
        }
        if instruction.onbuild {
            continue;
        }
        if let Some(script) = script {
            run(&mut state, step, script, findings);
        } else if instruction.keyword == Keyword::From {
            enter_stage(&mut state, BaseImage::parse(instruction));
        } else if instruction.keyword == Keyword::Shell {
            state.pipefail = shell_handles_pipes(&instruction.script());
        } else if instruction.keyword == Keyword::Env && disables_pip_cache(&instruction.arguments) {
            state.pip_cache_disabled.mark();
        }
    }
    for (line, image) in &state.uncleaned_lists {
        let parent = image
            .as_ref()
            .and_then(|image| image.alias.as_ref())
            .is_some_and(|alias| state.parent_images.contains(alias));
        if *image == state.current_image || parent {
            findings.report(DockerfileRule::Dl3009, *line, String::new());
        }
    }
}

fn disables_pip_cache(arguments: &str) -> bool {
    environment_pairs(arguments.trim())
        .iter()
        .any(|(name, value)| name == PIP_NO_CACHE_VARIABLE && TRUTHY.contains(&value.as_str()))
}

fn enter_stage(state: &mut RunState<'_>, image: BaseImage) {
    state.pipefail = false;
    state.downloaders.clear();
    state.parent_images.insert(image.name.clone());
    state.pip_cache_disabled.enter(&image);
    state.current_image = Some(image);
}

fn consecutive_runs<'steps>(state: &mut RunState<'steps>, step: &'steps Step, findings: &mut Findings) {
    let commands = step.script.as_ref().map_or(0, |script| script.commands.len());
    let flags = step.instruction.flags.as_slice();
    let consolidable = state
        .previous_run
        .is_some_and(|(previous_flags, previous_commands)| {
            previous_flags == flags && commands <= SINGLE_COMMAND && previous_commands <= SINGLE_COMMAND
        });
    if consolidable {
        findings.report(DockerfileRule::Dl3059, step.instruction.line, String::new());
    } else {
        state.previous_run = Some((flags, commands));
    }
}

fn shell_handles_pipes(shell: &str) -> bool {
    if NON_POSIX_SHELLS.iter().any(|name| shell.starts_with(name)) {
        return true;
    }
    super::super::shell::parse(shell).any(|command| {
        PIPEFAIL_SHELLS.contains(&command.name.as_str())
            && command.has_flag("o")
            && command.has_argument(PIPEFAIL)
    })
}

fn run(state: &mut RunState<'_>, step: &Step, script: &Script, findings: &mut Findings) {
    let instruction = &step.instruction;
    let line = instruction.line;
    if script.has_pipe && !state.pipefail {
        findings.report(DockerfileRule::Dl4006, line, String::new());
    }
    let used: Vec<&'static str> = DOWNLOADERS
        .iter()
        .copied()
        .filter(|downloader| script.uses(downloader))
        .collect();
    state.downloaders.extend(used.iter().copied());
    if !used.is_empty() && state.downloaders.len() >= DOWNLOADERS.len() {
        findings.report(DockerfileRule::Dl4001, line, String::new());
    }
    let cached_lists = has_caching_mount(instruction, APT_LISTS)
        || (has_caching_mount(instruction, APT_DIRECTORY) && has_caching_mount(instruction, APT_CACHE));
    if forgets_apt_lists(script) && !cached_lists {
        state.uncleaned_lists.push((line, state.current_image.clone()));
    }
    if keeps_pip_cache(state, &instruction.script(), script) && !has_caching_mount(instruction, PIP_CACHE) {
        findings.report(DockerfileRule::Dl3042, line, String::new());
    }
}

fn forgets_apt_lists(script: &Script) -> bool {
    let updates = script.any(|command| {
        PACKAGE_MANAGERS
            .iter()
            .any(|manager| command.has_any_argument(manager, &[UPDATE]))
    });
    updates && !script.any(|command| command.has_any_argument("rm", APT_LISTS_CLEANUP))
}

fn keeps_pip_cache(state: &RunState<'_>, text: &str, script: &Script) -> bool {
    if state.pip_cache_disabled.is_set() {
        return false;
    }
    let assigned = text
        .split_once(PIP_NO_CACHE_ASSIGNMENT)
        .is_some_and(|(_, value)| TRUTHY.iter().any(|truthy| value.starts_with(truthy)));
    !assigned && script.any(caches_pip)
}

fn caches_pip(command: &Command) -> bool {
    let wrapper = PIP_WRAPPERS.iter().any(|wrapper| {
        command.name.contains(wrapper)
            || (command.name.starts_with(PYTHON_PREFIX)
                && command.arguments.windows(2).any(|pair| {
                    pair.first().is_some_and(|flag| flag == MODULE_FLAG)
                        && pair.get(1).is_some_and(|name| name == wrapper)
                }))
    });
    is_pip_install(command) && !command.has_argument(NO_CACHE_DIR) && !wrapper
}
