const SHEBANG: &str = "#!";
const ENV: &str = "env";
const ENV_PATH: &str = "/env ";
const ENV_SPLIT_FLAGS: &[&str] = &["--split-string=", "--split-string", "-S"];
const BUSYBOX: &str = "busybox";
const FLAG_PREFIX: char = '-';
const ENV_ASSIGNMENT: char = '=';
const PATH_SEPARATOR: char = '/';
const UNSUPPORTED_SHELLS: &[&str] = &[
    "awk", "csh", "expect", "fish", "perl", "python", "python3", "ruby", "tcsh", "zsh",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    Sh,
    Bash,
    Dash,
    Ksh,
    Busybox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpreter {
    Missing,
    Named(Option<Dialect>),
    Unsupported,
}

impl Dialect {
    pub fn from_executable(name: &str) -> Option<Self> {
        match name {
            "sh" => Some(Self::Sh),
            "bash" | "bats" => Some(Self::Bash),
            "busybox" | "busybox sh" | "busybox ash" => Some(Self::Busybox),
            "dash" | "ash" => Some(Self::Dash),
            "ksh" | "ksh88" | "ksh93" | "oksh" => Some(Self::Ksh),
            _ => None,
        }
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension {
            "bash" | "bats" => Some(Self::Bash),
            "ksh" => Some(Self::Ksh),
            "dash" => Some(Self::Dash),
            _ => None,
        }
    }

    pub const fn parses_declaration_assignments(self) -> bool {
        !matches!(self, Self::Sh)
    }
}

pub fn interpreter(first_line: &str) -> Interpreter {
    let Some(shebang) = first_line.strip_prefix(SHEBANG) else {
        return Interpreter::Missing;
    };
    let executable = executable_from_shebang(shebang.trim());
    if UNSUPPORTED_SHELLS
        .iter()
        .any(|unsupported| executable.starts_with(unsupported))
    {
        return Interpreter::Unsupported;
    }
    Interpreter::Named(Dialect::from_executable(&executable))
}

fn executable_from_shebang(shebang: &str) -> String {
    if let Some((_, after_env)) = shebang.split_once(ENV_PATH) {
        let rest = after_env.trim_start();
        let rest = ENV_SPLIT_FLAGS
            .iter()
            .find_map(|flag| rest.strip_prefix(flag))
            .unwrap_or(rest);
        let words: Vec<&str> = rest.split_whitespace().collect();
        return from_env_arguments(&words);
    }
    let words: Vec<&str> = shebang.split_whitespace().collect();
    match words.as_slice() {
        [] => String::new(),
        [only] => basename(only).to_owned(),
        [first, rest @ ..] if basename(first) == ENV => from_env_arguments(rest),
        [first, second, ..] if basename(first) == BUSYBOX => match basename(second) {
            "sh" => "busybox sh".to_owned(),
            "ash" => "busybox ash".to_owned(),
            other => other.to_owned(),
        },
        [first, ..] => basename(first).to_owned(),
    }
}

fn from_env_arguments(arguments: &[&str]) -> String {
    arguments
        .iter()
        .copied()
        .skip_while(|argument| argument.starts_with(FLAG_PREFIX))
        .find(|argument| !argument.contains(ENV_ASSIGNMENT))
        .map(|argument| basename(argument).to_owned())
        .unwrap_or_default()
}

fn basename(path: &str) -> &str {
    path.rsplit(PATH_SEPARATOR).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::{Dialect, Interpreter, interpreter};

    #[test]
    fn reads_the_interpreter_like_shellcheck() {
        assert_eq!(interpreter("#!/bin/sh"), Interpreter::Named(Some(Dialect::Sh)));
        assert_eq!(
            interpreter("#!/usr/bin/env bash"),
            Interpreter::Named(Some(Dialect::Bash))
        );
        assert_eq!(
            interpreter("#! /bin/bash -e"),
            Interpreter::Named(Some(Dialect::Bash))
        );
        assert_eq!(
            interpreter("#!/usr/bin/env -S X=1 bash"),
            Interpreter::Named(Some(Dialect::Bash))
        );
        assert_eq!(
            interpreter("#!/usr/bin/env --split-string=bash -x"),
            Interpreter::Named(Some(Dialect::Bash))
        );
        assert_eq!(
            interpreter("#!/bin/busybox sh"),
            Interpreter::Named(Some(Dialect::Busybox))
        );
        assert_eq!(interpreter("#!/bin/ash"), Interpreter::Named(Some(Dialect::Dash)));
        assert_eq!(interpreter("#!/usr/bin/bash5"), Interpreter::Named(None));
        assert_eq!(interpreter("#!/bin/zsh"), Interpreter::Unsupported);
        assert_eq!(interpreter("#!/usr/bin/env python3"), Interpreter::Unsupported);
        assert_eq!(interpreter("echo hola"), Interpreter::Missing);
        assert_eq!(Dialect::from_extension("bash"), Some(Dialect::Bash));
        assert_eq!(Dialect::from_extension("sh"), None);
    }
}
