mod pip;

pub(super) use pip::is_pip_install;

use super::shell::{Command, Script};
use super::{Findings, Step, has_caching_mount};
use crate::rules::DockerfileRule;

const APT_GET: &str = "apt-get";
const APK: &str = "apk";
const NPM: &str = "npm";
const YUM: &str = "yum";
const WGET: &str = "wget";
const GO: &str = "go";
const INSTALL: &str = "install";
const ADD: &str = "add";
const APT_PINNED_MARKERS: &[&str] = &["=", "/"];
const DEB_SUFFIX: &str = ".deb";
const APK_PINNED_MARKERS: &[&str] = &["=", "~", ">", "<"];
const APK_SUFFIX: &str = ".apk";
const APK_CACHE: &str = "/var/cache/apk";
const NPM_IGNORED_FLAGS: &[&str] = &["loglevel", "registry"];
const NPM_GIT_PREFIXES: &[&str] = &["git://", "git+ssh://", "git+http://", "git+https://"];
const NPM_TARBALL_SUFFIXES: &[&str] = &[".tar", ".tar.gz", ".tgz"];
const NPM_PATH_PREFIXES: &[&str] = &["/", "./", "../", "~/"];
const SCOPE_MARKER: char = '@';
const GIT_REVISION_MARKER: char = '#';
const GO_TAGS: &[&str] = &["@latest", "@none"];

pub(super) fn check(steps: &[Step], findings: &mut Findings) {
    for step in steps {
        let Some(script) = &step.script else {
            continue;
        };
        let line = step.instruction.line;
        let mut report = |rule: DockerfileRule, subject: Vec<String>| {
            findings.report(rule, line, subject.join(" "));
        };
        for (rule, unpinned) in unpinned_packages(script) {
            if !unpinned.is_empty() {
                report(rule, unpinned);
            }
        }
        let flagged = [
            (
                DockerfileRule::Dl3014,
                script.any(|command| apt_install(command) && !assumes_yes(command)),
            ),
            (
                DockerfileRule::Dl3015,
                script.any(|command| apt_install(command) && !skips_recommends(command)),
            ),
            (
                DockerfileRule::Dl3019,
                !has_caching_mount(&step.instruction, APK_CACHE)
                    && script.any(|command| apk_add(command) && !command.has_flag("no-cache")),
            ),
            (DockerfileRule::Dl3027, script.uses("apt")),
            (DockerfileRule::Dl3032, misses_yum_clean(script)),
            (DockerfileRule::Dl3047, script.any(wget_without_progress)),
            (DockerfileRule::Dl3004, script.uses("sudo")),
            (
                DockerfileRule::Dl3003,
                !step.instruction.onbuild && script.uses("cd"),
            ),
        ];
        for (rule, failed) in flagged {
            if failed {
                report(rule, Vec::new());
            }
        }
    }
}

fn unpinned_packages(script: &Script) -> [(DockerfileRule, Vec<String>); 5] {
    [
        (DockerfileRule::Dl3008, apt_unpinned(script)),
        (DockerfileRule::Dl3018, apk_unpinned(script)),
        (DockerfileRule::Dl3013, pip::unpinned(script)),
        (DockerfileRule::Dl3016, npm_unpinned(script)),
        (DockerfileRule::Dl3062, go_unpinned(script)),
    ]
}

fn apt_install(command: &Command) -> bool {
    command.has_any_argument(APT_GET, &[INSTALL])
}

fn apk_add(command: &Command) -> bool {
    command.has_any_argument(APK, &[ADD])
}

fn apt_unpinned(script: &Script) -> Vec<String> {
    script
        .commands
        .iter()
        .filter(|command| apt_install(command))
        .flat_map(|command| packages_without(command, &["t", "target-release"], INSTALL))
        .filter(|package| {
            !(APT_PINNED_MARKERS.iter().any(|marker| package.contains(marker))
                || package.ends_with(DEB_SUFFIX))
        })
        .collect()
}

fn apk_unpinned(script: &Script) -> Vec<String> {
    script
        .commands
        .iter()
        .filter(|command| apk_add(command))
        .flat_map(|command| packages_without(command, &["t", "virtual", "repository", "X"], ADD))
        .filter(|package| {
            !(APK_PINNED_MARKERS.iter().any(|marker| package.contains(marker))
                || package.ends_with(APK_SUFFIX))
        })
        .collect()
}

fn packages_without(command: &Command, flags: &[&str], verb: &str) -> Vec<String> {
    command
        .without_flag_values(flags)
        .arguments_without_flags()
        .into_iter()
        .filter(|argument| *argument != verb)
        .map(str::to_owned)
        .collect()
}

fn npm_unpinned(script: &Script) -> Vec<String> {
    script
        .commands
        .iter()
        .filter(|command| command.has_any_argument(NPM, &[INSTALL]))
        .flat_map(|command| {
            let arguments = packages_without(command, NPM_IGNORED_FLAGS, "");
            let starts_with_install = arguments.first().is_some_and(|first| first == INSTALL);
            let packages = if starts_with_install {
                after_verb(&arguments, INSTALL)
            } else {
                Vec::new()
            };
            packages.into_iter().filter(|package| !npm_pinned(package))
        })
        .collect()
}

pub(super) fn after_verb(arguments: &[String], verb: &str) -> Vec<String> {
    arguments
        .iter()
        .skip_while(|argument| *argument != verb)
        .skip_while(|argument| *argument == verb)
        .cloned()
        .collect()
}

fn npm_pinned(package: &str) -> bool {
    if NPM_GIT_PREFIXES.iter().any(|prefix| package.starts_with(prefix)) {
        return package.contains(GIT_REVISION_MARKER);
    }
    if NPM_TARBALL_SUFFIXES
        .iter()
        .any(|suffix| package.ends_with(suffix))
        || NPM_PATH_PREFIXES.iter().any(|prefix| package.starts_with(prefix))
    {
        return true;
    }
    let unscoped = if package.starts_with(SCOPE_MARKER) {
        package.trim_start_matches(|character: char| character != '/')
    } else {
        package
    };
    unscoped.contains(SCOPE_MARKER)
}

fn go_unpinned(script: &Script) -> Vec<String> {
    let run: Vec<&str> = script
        .commands
        .iter()
        .filter(|command| command.has_any_argument(GO, &["run"]))
        .flat_map(|command| command.arguments_without_flags().into_iter().take(2))
        .filter(|argument| *argument != "run")
        .collect();
    let packages = if run.is_empty() {
        script
            .commands
            .iter()
            .filter(|command| command.has_any_argument(GO, &[INSTALL, "get"]))
            .flat_map(Command::arguments_without_flags)
            .filter(|argument| ![INSTALL, "get", "tool"].contains(argument))
            .collect()
    } else {
        run
    };
    packages
        .into_iter()
        .filter(|package| {
            let local = *package == "." || package.starts_with('/') || package.starts_with('.');
            let versioned =
                package.contains(SCOPE_MARKER) && !GO_TAGS.iter().any(|tag| package.ends_with(tag));
            !(local || versioned)
        })
        .map(str::to_owned)
        .collect()
}

fn assumes_yes(command: &Command) -> bool {
    command.has_any_flag(&["y", "yes", "qq", "assume-yes"])
        || command.flag_count("q") == 2
        || command.flag_count("quiet") == 2
        || command.has_argument("-q=2")
}

fn skips_recommends(command: &Command) -> bool {
    command.has_flag("no-install-recommends") || command.has_argument("APT::Install-Recommends=false")
}

fn misses_yum_clean(script: &Script) -> bool {
    let install = script.position(|command| command.has_any_argument(YUM, &[INSTALL]));
    let clean = script.position(|command| {
        command.has_any_argument(YUM, &["clean", "all"])
            || command.has_any_argument("rm", &["-rf", "/var/cache/yum/*"])
    });
    match (install, clean) {
        (None, _) => false,
        (Some(install), Some(clean)) => install >= clean,
        (Some(_), None) => true,
    }
}

fn wget_without_progress(command: &Command) -> bool {
    command.is(WGET)
        && !command.has_flag("progress")
        && !command.has_any_flag(&[
            "q",
            "quiet",
            "o",
            "output-file",
            "a",
            "append-output",
            "no-verbose",
        ])
        && !command.has_argument("-nv")
}
