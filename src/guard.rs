mod markers;
mod overlap;
mod settings;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use tree_sitter::Parser;

use crate::config::Config;
use crate::diff::{ChangedLines, Changes};
use crate::i18n::{Cheat, Message};
use crate::language::{Language, Stack};
use crate::rules::{Diagnostic, RuleId};
use crate::suppressions::directives;

use self::markers::{Marker, MarkerKind, directive_marker, find_markers};

pub use self::overlap::add_guarded;

const FIRST_LINE: usize = 1;
const FIRST_COLUMN: usize = 1;
const ALLOWED_OCCURRENCES: usize = 0;

#[derive(Debug)]
pub struct GuardRequest<'request> {
    pub changes: &'request ChangedLines,
    pub config: &'request Config,
    pub project_root: &'request Path,
    pub stacks: &'request [Stack],
}

struct Inspected<'file> {
    path: &'file Path,
    relative: String,
    language: Language,
}

pub fn inspect(request: &GuardRequest<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut counts = Vec::new();
    for (path, changes) in request.changes.files() {
        if let Some(file) = inspected(path, request) {
            let (found, count) = inspect_changed(file, changes, request);
            diagnostics.extend(found);
            counts.extend(count);
        }
    }
    for path in request.changes.deleted() {
        if let Some(file) = inspected(path, request) {
            counts.extend(test_count(file, None, request));
        }
    }
    diagnostics.extend(removed_tests(&counts));
    diagnostics.extend(settings::loosened_settings(
        request.changes,
        request.config,
        request.project_root,
    ));
    diagnostics
        .sort_by(|left, right| (&left.relative_path, left.line).cmp(&(&right.relative_path, right.line)));
    diagnostics
}

fn inspected<'file>(path: &'file Path, request: &GuardRequest<'_>) -> Option<Inspected<'file>> {
    let language = Language::from_path(path)?;
    let relative = path
        .strip_prefix(request.project_root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let wanted = request.stacks.is_empty() || request.stacks.contains(&language.stack());
    (wanted && !request.config.is_ignored(Path::new(&relative))).then_some(Inspected {
        path,
        relative,
        language,
    })
}

fn inspect_changed<'file>(
    file: Inspected<'file>,
    changes: &Changes,
    request: &GuardRequest<'_>,
) -> (Vec<Diagnostic>, Option<TestCount<'file>>) {
    let Ok(source) = fs::read_to_string(file.path) else {
        return (Vec::new(), None);
    };
    let markers = markers_of(&source, file.language);
    let mut occurrences: HashMap<(RuleId, String), usize> = HashMap::new();
    let diagnostics: Vec<Diagnostic> = markers
        .iter()
        .filter(|marker| {
            if marker.kind == MarkerKind::EmptyTest {
                changes.touches_span(marker.line, marker.end_line)
            } else {
                changes.adds_line(marker.line)
            }
        })
        .filter_map(|marker| {
            let rule = rule_for(marker.kind)?;
            enabled(&file, rule, request).then(|| {
                let occurrence = occurrences.entry((rule, marker.text.clone())).or_default();
                *occurrence = occurrence.saturating_add(1);
                marker_diagnostic(&file, rule, marker, *occurrence)
            })
        })
        .collect();
    (diagnostics, test_count(file, Some(&markers), request))
}

struct TestCount<'file> {
    file: Inspected<'file>,
    before: usize,
    after: usize,
}

fn test_count<'file>(
    file: Inspected<'file>,
    current: Option<&[Marker]>,
    request: &GuardRequest<'_>,
) -> Option<TestCount<'file>> {
    if !file.language.is_test_path(Path::new(&file.relative)) || !enabled(&file, RuleId::TestRemoved, request)
    {
        return None;
    }
    let before = request
        .changes
        .previous_version(file.path)
        .map_or(0, |previous| count_tests(&markers_of(&previous, file.language)));
    let after = current.map_or(0, count_tests);
    Some(TestCount { file, before, after })
}

fn removed_tests(counts: &[TestCount<'_>]) -> Vec<Diagnostic> {
    let before: usize = counts.iter().map(|count| count.before).sum();
    let after: usize = counts.iter().map(|count| count.after).sum();
    if after >= before {
        return Vec::new();
    }
    let net = before - after;
    counts
        .iter()
        .filter(|count| count.after < count.before)
        .map(|count| {
            let removed = count.before - count.after;
            Diagnostic {
                message: Message::Cheat(Cheat::TestsRemoved {
                    removed,
                    before: count.before,
                    after: count.after,
                    net,
                }),
                value: removed,
                ..base_diagnostic(
                    &count.file,
                    RuleId::TestRemoved,
                    FIRST_LINE,
                    format!("test-removed:{}", count.file.relative),
                )
            }
        })
        .collect()
}

fn count_tests(markers: &[Marker]) -> usize {
    markers
        .iter()
        .filter(|marker| marker.kind == MarkerKind::TestDefinition)
        .count()
}

fn markers_of(source: &str, language: Language) -> Vec<Marker> {
    let mut markers = syntax_markers(source, language);
    let marked: HashSet<usize> = markers
        .iter()
        .filter(|marker| marker.kind == MarkerKind::Suppression)
        .map(|marker| marker.line)
        .collect();
    markers.extend(
        directives(source, language == Language::Go)
            .into_iter()
            .filter(|directive| !marked.contains(&directive.line))
            .map(|directive| directive_marker(directive.line, &directive.text)),
    );
    markers
}

fn syntax_markers(source: &str, language: Language) -> Vec<Marker> {
    let (Some(grammar), Some(profile)) = (language.grammar(), language.profile()) else {
        return Vec::new();
    };
    let mut parser = Parser::new();
    if parser.set_language(&grammar).is_err() {
        return Vec::new();
    }
    parser.parse(source, None).map_or_else(Vec::new, |tree| {
        find_markers(tree.root_node(), source.as_bytes(), profile)
    })
}

const fn rule_for(kind: MarkerKind) -> Option<RuleId> {
    match kind {
        MarkerKind::Suppression => Some(RuleId::SuppressionAdded),
        MarkerKind::DisabledTest | MarkerKind::EmptyTest => Some(RuleId::TestDisabled),
        MarkerKind::TypeEscape => Some(RuleId::TypeEscape),
        MarkerKind::TestDefinition => None,
    }
}

fn enabled(file: &Inspected<'_>, rule: RuleId, request: &GuardRequest<'_>) -> bool {
    let is_test = file.language.is_test_path(Path::new(&file.relative));
    request
        .config
        .limits_for(Path::new(&file.relative), file.language, is_test)
        .get(rule)
        .is_some()
}

fn marker_diagnostic(file: &Inspected<'_>, rule: RuleId, marker: &Marker, occurrence: usize) -> Diagnostic {
    let fingerprint = format!("{}:{}:{}#{occurrence}", rule.as_str(), file.relative, marker.text);
    Diagnostic {
        message: marker_message(rule, marker),
        ..base_diagnostic(file, rule, marker.line, fingerprint)
    }
}

fn marker_message(rule: RuleId, marker: &Marker) -> Message {
    let text = marker.text.clone();
    if marker.kind == MarkerKind::EmptyTest {
        return Message::Cheat(Cheat::EmptyTest { name: text });
    }
    match rule {
        RuleId::TestDisabled => Message::Cheat(Cheat::TestSkipped { marker: text }),
        RuleId::TypeEscape => Message::Cheat(Cheat::TypeEscaped { marker: text }),
        RuleId::SuppressionAdded
        | RuleId::FileLength
        | RuleId::FunctionLength
        | RuleId::CyclomaticComplexity
        | RuleId::CognitiveComplexity
        | RuleId::ParameterCount
        | RuleId::TestRemoved
        | RuleId::NestingDepth
        | RuleId::MagicNumber
        | RuleId::ShortName
        | RuleId::DuplicateCode
        | RuleId::CommentedCode
        | RuleId::UnusedImport
        | RuleId::RedundantImport
        | RuleId::StarImport
        | RuleId::RedundantModifier
        | RuleId::FinalParameter
        | RuleId::FinalClass
        | RuleId::Migration(_)
        | RuleId::Dockerfile(_)
        | RuleId::Compose(_)
        | RuleId::CommittedSecret
        | RuleId::VulnerableDependency
        | RuleId::Python(_)
        | RuleId::Eslint(_)
        | RuleId::Shell(_)
        | RuleId::Go(_)
        | RuleId::Java(_)
        | RuleId::Taint(_) => Message::Cheat(Cheat::SuppressionAdded { marker: text }),
    }
}

fn base_diagnostic(file: &Inspected<'_>, rule: RuleId, line: usize, fingerprint: String) -> Diagnostic {
    Diagnostic {
        path: file.relative.clone(),
        line,
        column: FIRST_COLUMN,
        end_line: line,
        rule,
        message: Message::Rule(rule),
        value: 1,
        limit: ALLOWED_OCCURRENCES,
        fingerprint,
        relative_path: file.relative.clone(),
        absolute_path: file.path.to_path_buf(),
        fix: None,
    }
}

#[cfg(test)]
mod tests;
