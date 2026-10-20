use super::after_verb;
use crate::deploy::dockerfile::shell::{Command, Script};

const PIP_PREFIX: &str = "pip";
const PIPENV_PREFIX: &str = "pipenv";
const PYTHON_PREFIX: &str = "python";
const INSTALL: &str = "install";
const MODULE_INSTALL: [&str; 3] = ["-m", "pip", "install"];
const REQUIREMENT_ARGUMENTS: &[&str] = &["--requirement", "-r", "."];
const CONSTRAINT_FLAGS: &[&str] = &["constraint", "c"];
const VALUED_FLAGS: &[&str] = &[
    "abi",
    "b",
    "build",
    "e",
    "editable",
    "extra-index-url",
    "f",
    "find-links",
    "i",
    "index-url",
    "implementation",
    "no-binary",
    "only-binary",
    "platform",
    "prefix",
    "progress-bar",
    "proxy",
    "python",
    "python-version",
    "root",
    "root-user-action",
    "src",
    "t",
    "target",
    "trusted-host",
    "upgrade-strategy",
];
const VERSION_SYMBOLS: &[&str] = &["==", ">=", "<=", ">", "<", "!=", "~=", "==="];
const LOCAL_SUFFIXES: &[&str] = &[".whl", ".tar.gz"];
const VCS_SCHEMES: &[&str] = &[
    "git+file",
    "git+https",
    "git+ssh",
    "git+http",
    "git+git",
    "git",
    "hg+file",
    "hg+http",
    "hg+https",
    "hg+ssh",
    "hg+static-http",
    "svn",
    "svn+svn",
    "svn+http",
    "svn+https",
    "svn+ssh",
    "bzr+http",
    "bzr+https",
    "bzr+ssh",
    "bzr+sftp",
    "bzr+ftp",
    "bzr+lp",
];
const VERSION_MARKER: char = '@';
const PATH_MARKER: char = '/';

pub fn is_pip_install(command: &Command) -> bool {
    let pip = command.name.starts_with(PIP_PREFIX)
        && !command.name.starts_with(PIPENV_PREFIX)
        && command.has_argument(INSTALL);
    let module = command.name.starts_with(PYTHON_PREFIX)
        && command.arguments.windows(MODULE_INSTALL.len()).any(|window| {
            window
                .iter()
                .zip(MODULE_INSTALL)
                .all(|(argument, expected)| argument == expected)
        });
    pip || module
}

pub fn unpinned(script: &Script) -> Vec<String> {
    script
        .commands
        .iter()
        .filter(|command| {
            is_pip_install(command)
                && !command.has_any_flag(CONSTRAINT_FLAGS)
                && !REQUIREMENT_ARGUMENTS
                    .iter()
                    .any(|argument| command.has_argument(argument))
        })
        .flat_map(|command| {
            let arguments: Vec<String> = command
                .without_flag_values(VALUED_FLAGS)
                .arguments_without_flags()
                .into_iter()
                .map(str::to_owned)
                .collect();
            after_verb(&arguments, INSTALL)
        })
        .filter(|package| !pinned(package))
        .collect()
}

fn pinned(package: &str) -> bool {
    let is_vcs = VCS_SCHEMES.iter().any(|scheme| package.starts_with(scheme));
    VERSION_SYMBOLS.iter().any(|symbol| package.contains(symbol))
        || (is_vcs && package.contains(VERSION_MARKER))
        || LOCAL_SUFFIXES.iter().any(|suffix| package.ends_with(suffix))
        || (!is_vcs && package.contains(PATH_MARKER))
}
