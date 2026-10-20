use std::sync::OnceLock;

use aho_corasick::AhoCorasick;
use regex::bytes::{Regex, RegexBuilder};
use serde::Deserialize;

use super::allowlist::{Allowlist, RawAllowlist};

const DEFAULT_RULES: &str = include_str!("gitleaks.toml");
const EXTRA_RULES: &str = include_str!("extra-rules.toml");
const ALTERNATION: &str = "|";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCatalog;

#[derive(Debug, Deserialize)]
struct RawCatalog {
    #[serde(default)]
    allowlist: RawAllowlist,
    rules: Vec<RawRule>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
enum Threshold {
    Whole(u8),
    Fractional(f64),
}

impl Threshold {
    fn value(self) -> f64 {
        match self {
            Self::Whole(whole) => f64::from(whole),
            Self::Fractional(fractional) => fractional,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRule {
    id: String,
    regex: Option<String>,
    entropy: Option<Threshold>,
    #[serde(default)]
    keywords: Vec<String>,
    path: Option<String>,
    secret_group: Option<usize>,
    #[serde(default)]
    allowlists: Vec<RawAllowlist>,
}

#[derive(Debug)]
pub struct SecretRule {
    pub id: String,
    pub source: Option<String>,
    pub pattern: OnceLock<Option<Regex>>,
    pub entropy: Option<f64>,
    pub path: Option<usize>,
    pub secret_group: Option<usize>,
    pub allowlists: Vec<Allowlist>,
}

impl SecretRule {
    pub fn pattern(&self) -> Option<&Regex> {
        self.pattern
            .get_or_init(|| self.source.as_deref().and_then(|source| compile(source).ok()))
            .as_ref()
    }

    pub const fn has_pattern(&self) -> bool {
        self.source.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathProfile {
    matches: Vec<bool>,
}

impl PathProfile {
    pub fn matches(&self, path: usize) -> bool {
        self.matches.get(path).copied().unwrap_or_default()
    }

    pub fn signature(&self) -> &[bool] {
        &self.matches
    }
}

#[derive(Debug, Default)]
pub struct PathTable {
    patterns: Vec<Regex>,
}

impl PathTable {
    pub fn add(&mut self, sources: &[String]) -> Result<Option<usize>, InvalidCatalog> {
        if sources.is_empty() {
            return Ok(None);
        }
        self.patterns.push(compile(&join(sources))?);
        Ok(Some(self.patterns.len().saturating_sub(1)))
    }
}

#[derive(Debug, Default)]
struct KeywordIndex {
    words: Vec<String>,
    rules: Vec<Vec<usize>>,
    unkeyed: Vec<usize>,
}

impl KeywordIndex {
    fn add(&mut self, rule: usize, keywords: &[String]) {
        if keywords.is_empty() {
            self.unkeyed.push(rule);
        }
        for keyword in keywords {
            let lowered = keyword.to_lowercase();
            let known = self.words.iter().position(|word| *word == lowered);
            if let Some(rules) = known.and_then(|slot| self.rules.get_mut(slot)) {
                rules.push(rule);
            } else {
                self.words.push(lowered);
                self.rules.push(vec![rule]);
            }
        }
    }
}

#[derive(Debug)]
pub struct Catalog {
    global: Allowlist,
    rules: Vec<SecretRule>,
    automaton: AhoCorasick,
    keywords: KeywordIndex,
    paths: Vec<Regex>,
}

pub fn shared() -> Option<&'static Catalog> {
    static SHARED: OnceLock<Option<Catalog>> = OnceLock::new();
    SHARED
        .get_or_init(|| Catalog::parse(DEFAULT_RULES, EXTRA_RULES).ok())
        .as_ref()
}

impl Catalog {
    pub fn parse(primary: &str, extra: &str) -> Result<Self, InvalidCatalog> {
        let raw: RawCatalog = toml::from_str(primary).map_err(|_| InvalidCatalog)?;
        let extra: RawCatalog = toml::from_str(extra).map_err(|_| InvalidCatalog)?;
        let mut table = PathTable::default();
        let global = Allowlist::build(&raw.allowlist, &mut table)?;
        let mut keywords = KeywordIndex::default();
        let mut rules = Vec::with_capacity(raw.rules.len());
        for (index, rule) in raw.rules.into_iter().chain(extra.rules).enumerate() {
            keywords.add(index, &rule.keywords);
            rules.push(build_rule(rule, &mut table)?);
        }
        Ok(Self {
            global,
            rules,
            automaton: AhoCorasick::new(&keywords.words).map_err(|_| InvalidCatalog)?,
            keywords,
            paths: table.patterns,
        })
    }

    pub fn profile(&self, relative: &str) -> Option<PathProfile> {
        let profile = PathProfile {
            matches: self
                .paths
                .iter()
                .map(|pattern| pattern.is_match(relative.as_bytes()))
                .collect(),
        };
        (!self.global.skips_file(&profile)).then_some(profile)
    }

    pub const fn global(&self) -> &Allowlist {
        &self.global
    }

    pub fn rules(&self) -> &[SecretRule] {
        &self.rules
    }

    pub fn rule_id(&self, rule: usize) -> Option<&str> {
        self.rules.get(rule).map(|rule| rule.id.as_str())
    }

    pub fn triggered_rules(&self, lowered: &[u8]) -> Vec<usize> {
        let mut triggered = vec![false; self.rules.len()];
        let mut mark = |rule: usize| {
            if let Some(slot) = triggered.get_mut(rule) {
                *slot = true;
            }
        };
        self.keywords.unkeyed.iter().copied().for_each(&mut mark);
        for found in self.automaton.find_overlapping_iter(lowered) {
            let rules = self.keywords.rules.get(found.pattern().as_usize());
            rules.into_iter().flatten().copied().for_each(&mut mark);
        }
        triggered
            .iter()
            .enumerate()
            .filter_map(|(rule, &on)| on.then_some(rule))
            .collect()
    }
}

fn build_rule(raw: RawRule, table: &mut PathTable) -> Result<SecretRule, InvalidCatalog> {
    let path = table.add(raw.path.as_slice())?;
    let allowlists = raw
        .allowlists
        .iter()
        .map(|allowlist| Allowlist::build(allowlist, table))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SecretRule {
        id: raw.id,
        source: raw.regex,
        pattern: OnceLock::new(),
        entropy: raw.entropy.map(Threshold::value),
        path,
        secret_group: raw.secret_group,
        allowlists,
    })
}

pub fn compile(pattern: &str) -> Result<Regex, InvalidCatalog> {
    RegexBuilder::new(&super::dialect::to_rust(pattern))
        .unicode(false)
        .build()
        .map_err(|_| InvalidCatalog)
}

pub fn join(patterns: &[String]) -> String {
    format!("(?:{})", patterns.join(ALTERNATION))
}
