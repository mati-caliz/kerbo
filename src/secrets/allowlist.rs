use aho_corasick::AhoCorasick;
use regex::bytes::Regex;
use serde::Deserialize;

use super::catalog::{InvalidCatalog, PathProfile, PathTable, compile, join};

const ALL_CONDITIONS: [&str; 2] = ["AND", "&&"];
const MATCH_TARGET: &str = "match";
const LINE_TARGET: &str = "line";

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAllowlist {
    condition: Option<String>,
    regex_target: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    regexes: Vec<String>,
    #[serde(default)]
    stopwords: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Secret,
    Match,
    Line,
}

#[derive(Debug, Clone, Copy)]
pub struct Evidence<'text> {
    pub secret: &'text [u8],
    pub matched: &'text [u8],
    pub line: &'text [u8],
}

#[derive(Debug)]
pub struct Allowlist {
    every_check: bool,
    target: Target,
    path: Option<usize>,
    regex: Option<Regex>,
    stopwords: Option<AhoCorasick>,
}

impl Allowlist {
    pub fn build(raw: &RawAllowlist, table: &mut PathTable) -> Result<Self, InvalidCatalog> {
        let every_check = raw
            .condition
            .as_deref()
            .is_some_and(|condition| ALL_CONDITIONS.contains(&condition.to_uppercase().as_str()));
        let target = match raw.regex_target.as_deref() {
            Some(MATCH_TARGET) => Target::Match,
            Some(LINE_TARGET) => Target::Line,
            _ => Target::Secret,
        };
        let regex = if raw.regexes.is_empty() {
            None
        } else {
            Some(compile(&join(&raw.regexes))?)
        };
        let stopwords = if raw.stopwords.is_empty() {
            None
        } else {
            let lowered: Vec<String> = raw.stopwords.iter().map(|word| word.to_lowercase()).collect();
            Some(AhoCorasick::new(lowered).map_err(|_| InvalidCatalog)?)
        };
        Ok(Self {
            every_check,
            target,
            path: table.add(&raw.paths)?,
            regex,
            stopwords,
        })
    }

    pub fn skips_file(&self, profile: &PathProfile) -> bool {
        let path_allowed = self.path.is_some_and(|path| profile.matches(path));
        if !self.every_check {
            return path_allowed;
        }
        self.regex.is_none() && self.stopwords.is_none() && self.path.is_none_or(|_| path_allowed)
    }

    pub fn skips_finding(&self, evidence: &Evidence<'_>, profile: &PathProfile) -> bool {
        let target = match self.target {
            Target::Secret => evidence.secret,
            Target::Match => evidence.matched,
            Target::Line => evidence.line,
        };
        let regex_allowed =
            !target.is_empty() && self.regex.as_ref().is_some_and(|regex| regex.is_match(target));
        let stopword_found = !evidence.secret.is_empty()
            && self
                .stopwords
                .as_ref()
                .is_some_and(|words| words.is_match(&evidence.secret.to_ascii_lowercase()));
        if !self.every_check {
            return regex_allowed || stopword_found;
        }
        let checks = [
            self.path.map(|path| profile.matches(path)),
            self.regex.as_ref().map(|_| regex_allowed),
            self.stopwords.as_ref().map(|_| stopword_found),
        ];
        checks.into_iter().flatten().all(|passed| passed)
    }
}
