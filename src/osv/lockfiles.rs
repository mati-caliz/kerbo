mod go;
mod maven;
mod npm;
mod pnpm;
mod python;
mod toml_blocks;
mod yarn;

use std::path::Path;

use super::Ecosystem;

const REQUIREMENTS_PREFIX: &str = "requirements";
const REQUIREMENTS_EXTENSION: &str = ".txt";
const FIRST_LINE: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub ecosystem: Ecosystem,
    pub name: String,
    pub version: String,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    CargoLock,
    PackageLock,
    PnpmLock,
    YarnLock,
    GoMod,
    GoSum,
    PoetryLock,
    UvLock,
    PipfileLock,
    Requirements,
    Pom,
    GradleLock,
}

impl Format {
    pub fn of(file_name: &str) -> Option<Self> {
        let format = match file_name {
            "Cargo.lock" => Self::CargoLock,
            "package-lock.json" | "npm-shrinkwrap.json" => Self::PackageLock,
            "pnpm-lock.yaml" => Self::PnpmLock,
            "yarn.lock" => Self::YarnLock,
            "go.mod" => Self::GoMod,
            "go.sum" => Self::GoSum,
            "poetry.lock" => Self::PoetryLock,
            "uv.lock" => Self::UvLock,
            "Pipfile.lock" => Self::PipfileLock,
            "pom.xml" => Self::Pom,
            "gradle.lockfile" | "buildscript-gradle.lockfile" => Self::GradleLock,
            _ if file_name.starts_with(REQUIREMENTS_PREFIX)
                && file_name.ends_with(REQUIREMENTS_EXTENSION) =>
            {
                Self::Requirements
            }
            _ => return None,
        };
        Some(format)
    }

    pub fn parse(self, path: &Path, text: &str) -> Vec<Dependency> {
        let dependencies = match self {
            Self::CargoLock => toml_blocks::packages(text, Ecosystem::CratesIo),
            Self::PoetryLock | Self::UvLock => toml_blocks::packages(text, Ecosystem::PyPi),
            Self::PackageLock => npm::package_lock(text),
            Self::PnpmLock => pnpm::lockfile(text),
            Self::YarnLock => yarn::lockfile(text),
            Self::GoMod => go::module_file(text),
            Self::GoSum => go::checksums(text),
            Self::PipfileLock => python::pipfile_lock(text),
            Self::Requirements => python::requirements(text),
            Self::Pom => maven::pom(path, text),
            Self::GradleLock => maven::gradle_lockfile(text),
        };
        dependencies
            .into_iter()
            .filter(|dependency| !dependency.name.is_empty() && !dependency.version.is_empty())
            .collect()
    }
}

pub fn line_of(text: &str, offset: usize) -> usize {
    text.get(..offset).map_or(FIRST_LINE, |before| {
        before.matches('\n').count().saturating_add(FIRST_LINE)
    })
}

pub fn first_line_containing(text: &str, needle: &str) -> usize {
    text.find(needle)
        .map_or(FIRST_LINE, |offset| line_of(text, offset))
}

pub fn numbered_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(index, line)| (index.saturating_add(FIRST_LINE), line))
}

#[cfg(test)]
mod tests;
