use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::language::{Language, Stack};
use crate::limits::{Limits, Preset};
use crate::rules::RuleId;

pub const CONFIG_FILE: &str = "kerbo.toml";

#[derive(Debug)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    preset: Preset,
    #[serde(default)]
    limits: Vec<RawLimit>,
    #[serde(default)]
    exceptions: Vec<RawException>,
    #[serde(default)]
    ignore: Vec<RawIgnore>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLimit {
    rule: RuleId,
    max: Option<usize>,
    language: Option<Stack>,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawException {
    paths: Vec<String>,
    rule: Option<RuleId>,
    max: Option<usize>,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIgnore {
    paths: Vec<String>,
    reason: String,
}

#[derive(Debug)]
struct Adjustment {
    rule: RuleId,
    max: Option<usize>,
    stack: Option<Stack>,
}

#[derive(Debug)]
struct Exception {
    paths: GlobSet,
    rule: Option<RuleId>,
    max: Option<usize>,
}

#[derive(Debug)]
pub struct Config {
    root: PathBuf,
    preset: Preset,
    adjustments: Vec<Adjustment>,
    exceptions: Vec<Exception>,
    ignored: GlobSet,
    deviations: usize,
}

impl Config {
    pub fn load(explicit: Option<&Path>, start: &Path) -> Result<Self, ConfigError> {
        let found = explicit.map(Path::to_path_buf).or_else(|| {
            start
                .ancestors()
                .map(|directory| directory.join(CONFIG_FILE))
                .find(|path| path.is_file())
        });
        let Some(path) = found else {
            return Ok(Self::defaults(start.to_path_buf()));
        };
        let text = fs::read_to_string(&path).map_err(|error| fail(&path, &error.to_string()))?;
        let raw: RawConfig = toml::from_str(&text).map_err(|error| fail(&path, &error.to_string()))?;
        let root = path
            .parent()
            .map_or_else(|| start.to_path_buf(), Path::to_path_buf);
        Self::from_raw(raw, root, &path)
    }

    fn defaults(root: PathBuf) -> Self {
        Self {
            root,
            preset: Preset::default(),
            adjustments: Vec::new(),
            exceptions: Vec::new(),
            ignored: GlobSet::empty(),
            deviations: 0,
        }
    }

    fn from_raw(raw: RawConfig, root: PathBuf, source: &Path) -> Result<Self, ConfigError> {
        let reasons = raw
            .limits
            .iter()
            .map(|limit| &limit.reason)
            .chain(raw.exceptions.iter().map(|exception| &exception.reason))
            .chain(raw.ignore.iter().map(|ignore| &ignore.reason));
        if reasons.clone().any(|reason| reason.trim().is_empty()) {
            return Err(fail(
                source,
                "cada ajuste necesita un `reason` que diga por qué se afloja",
            ));
        }
        let deviations = reasons.count();
        let exceptions = raw
            .exceptions
            .iter()
            .map(|raw| exception(raw, source))
            .collect::<Result<_, _>>()?;
        let ignore_paths: Vec<String> = raw.ignore.into_iter().flat_map(|ignore| ignore.paths).collect();
        Ok(Self {
            preset: raw.preset,
            adjustments: raw
                .limits
                .into_iter()
                .map(|limit| Adjustment {
                    rule: limit.rule,
                    max: limit.max,
                    stack: limit.language,
                })
                .collect(),
            exceptions,
            ignored: glob_set(&ignore_paths, source)?,
            deviations,
            root,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub const fn deviations(&self) -> usize {
        self.deviations
    }

    pub fn is_ignored(&self, relative: &Path) -> bool {
        self.ignored.is_match(relative)
    }

    pub fn limits_for(&self, relative: &Path, language: Language, is_test: bool) -> Limits {
        let stack = language.stack();
        let adjusted = self
            .adjustments
            .iter()
            .filter(|adjustment| adjustment.stack.is_none_or(|only| only == stack))
            .fold(self.preset.limits(language, is_test), |limits, adjustment| {
                limits.with_rule(adjustment.rule, adjustment.max)
            });
        self.exceptions
            .iter()
            .filter(|exception| exception.paths.is_match(relative))
            .fold(adjusted, |limits, exception| {
                exception
                    .rule
                    .map_or_else(Limits::default, |rule| limits.with_rule(rule, exception.max))
            })
    }
}

fn exception(raw: &RawException, source: &Path) -> Result<Exception, ConfigError> {
    if raw.rule.is_none() && raw.max.is_some() {
        return Err(fail(
            source,
            "una excepción con `max` tiene que nombrar la `rule`",
        ));
    }
    Ok(Exception {
        paths: glob_set(&raw.paths, source)?,
        rule: raw.rule,
        max: raw.max,
    })
}

fn glob_set(patterns: &[String], source: &Path) -> Result<GlobSet, ConfigError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            Glob::new(pattern).map_err(|error| fail(source, &format!("patrón `{pattern}`: {error}")))?;
        builder.add(glob);
    }
    builder.build().map_err(|error| fail(source, &error.to_string()))
}

fn fail(path: &Path, detail: &str) -> ConfigError {
    ConfigError(format!("{}: {detail}", path.display()))
}

#[cfg(test)]
mod tests;
