use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::cache::AnalysisCache;
use crate::config::Config;
use crate::i18n::{Message, SecretLeak};
use crate::limits::Limits;
use crate::rules::{Diagnostic, FileIdentity, RuleId};
use crate::secrets::{PathProfile, SecretFinding, find_secrets, path_profile};

const LARGEST_PLAIN_FILE_BYTES: u64 = 32 * 1024 * 1024;
const FOUND_ONCE: usize = 1;
const NOTHING_ALLOWED: usize = 0;

pub fn profile(identity: &FileIdentity, config: &Config, limits: Option<Limits>) -> Option<PathProfile> {
    let enabled = limits.map_or_else(
        || config.reports(Path::new(&identity.relative), RuleId::CommittedSecret),
        |limits| limits.get(RuleId::CommittedSecret).is_some(),
    );
    enabled.then(|| path_profile(&identity.relative)).flatten()
}

pub fn plain_file(
    identity: &FileIdentity,
    profile: Option<&PathProfile>,
    cache: Option<&AnalysisCache>,
) -> Result<Vec<Diagnostic>, String> {
    let Some(profile) = profile else {
        return Ok(Vec::new());
    };
    let size = fs::metadata(&identity.absolute)
        .map_err(|error| error.to_string())?
        .len();
    if size > LARGEST_PLAIN_FILE_BYTES {
        return Ok(Vec::new());
    }
    let source = fs::read(&identity.absolute).map_err(|error| error.to_string())?;
    Ok(diagnostics(identity, profile, &source, cache))
}

pub fn diagnostics(
    identity: &FileIdentity,
    profile: &PathProfile,
    source: &[u8],
    cache: Option<&AnalysisCache>,
) -> Vec<Diagnostic> {
    let findings = cached_or_fresh(profile, source, cache);
    let mut occurrences: HashMap<&str, usize> = HashMap::new();
    findings
        .iter()
        .map(|finding| {
            let occurrence = occurrences.entry(finding.kind.as_str()).or_default();
            *occurrence = occurrence.saturating_add(1);
            diagnostic(identity, finding, *occurrence)
        })
        .collect()
}

fn cached_or_fresh(
    profile: &PathProfile,
    source: &[u8],
    cache: Option<&AnalysisCache>,
) -> Vec<SecretFinding> {
    let Some(cache) = cache else {
        return find_secrets(source, profile);
    };
    let key = cache.secrets_key(profile, source);
    if let Some(findings) = cache.secrets(key) {
        return findings;
    }
    let findings = find_secrets(source, profile);
    cache.remember_secrets(key, &findings);
    findings
}

fn diagnostic(identity: &FileIdentity, finding: &SecretFinding, occurrence: usize) -> Diagnostic {
    Diagnostic {
        path: identity.display.clone(),
        line: finding.line,
        column: finding.column,
        end_line: finding.end_line,
        rule: RuleId::CommittedSecret,
        message: Message::CommittedSecret(SecretLeak {
            kind: finding.kind.clone(),
            preview: finding.preview.clone(),
            length: finding.length,
        }),
        value: FOUND_ONCE,
        limit: NOTHING_ALLOWED,
        fingerprint: format!(
            "{}:{}:{}#{occurrence}",
            RuleId::CommittedSecret.as_str(),
            identity.relative,
            finding.kind
        ),
        relative_path: identity.relative.clone(),
        absolute_path: identity.absolute.clone(),
        fix: None,
    }
}
