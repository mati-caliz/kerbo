mod alembic;
mod compose;
mod dockerfile;
mod sql;

use std::collections::HashMap;
use std::path::{Component, Path};

use tree_sitter::{Node, Parser};

use crate::analysis::ParseFailed;
use crate::fix::Fix;
use crate::i18n::Message;
use crate::language::Language;
use crate::limits::{Limits, Preset};
use crate::rules::{ComposeRule, Diagnostic, DockerfileRule, FileIdentity, MigrationRule, RuleId};
use dockerfile::severity::{Severity, severity};

const SQL_EXTENSION: &str = "sql";
const YAML_EXTENSIONS: &[&str] = &["yml", "yaml"];
const MIGRATION_DIRECTORIES: &[&str] = &["migrations", "migration", "drizzle"];
const FLYWAY_VERSIONED: &[char] = &['V', 'U'];
const FLYWAY_REPEATABLE: &str = "R__";
const FLYWAY_SEPARATOR: &str = "__";
const DOCKERFILE_NAMES: &[&str] = &["dockerfile", "containerfile"];
const DOCKERFILE_SUFFIX: &str = ".dockerfile";
const DOCKERIGNORE_SUFFIX: &str = ".dockerignore";
const COMPOSE_STEM: &str = "compose";
const DOCKER_COMPOSE_STEM: &str = "docker-compose";
const COMPOSE_VARIANT_SEPARATORS: &[char] = &['.', '-', '_'];
const FIRST_LINE: usize = 1;
const SINGLE_VIOLATION: usize = 1;
const NO_VIOLATIONS: usize = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: RuleId,
    pub line: usize,
    pub column: usize,
    pub subject: String,
    pub fix: Option<Fix>,
}

pub fn detect(path: &Path) -> Option<Language> {
    let name = path.file_name()?.to_str()?;
    let lowercase = name.to_lowercase();
    let extension = path.extension().and_then(|extension| extension.to_str());
    if extension == Some(SQL_EXTENSION) {
        return is_migration(path, name).then_some(Language::Sql);
    }
    if is_dockerfile(&lowercase) {
        return Some(Language::Dockerfile);
    }
    let is_yaml = extension.is_some_and(|extension| YAML_EXTENSIONS.contains(&extension));
    (is_yaml && is_compose(&lowercase)).then_some(Language::Compose)
}

fn is_migration(path: &Path, name: &str) -> bool {
    let in_migrations = path.components().any(|component| match component {
        Component::Normal(directory) => directory
            .to_str()
            .is_some_and(|directory| MIGRATION_DIRECTORIES.contains(&directory.to_lowercase().as_str())),
        Component::Prefix(_) | Component::RootDir | Component::CurDir | Component::ParentDir => false,
    });
    let versioned = name
        .strip_prefix(FLYWAY_VERSIONED)
        .is_some_and(|rest| rest.starts_with(|first: char| first.is_ascii_digit()));
    let flyway = (versioned || name.starts_with(FLYWAY_REPEATABLE)) && name.contains(FLYWAY_SEPARATOR);
    in_migrations || flyway
}

fn is_dockerfile(lowercase: &str) -> bool {
    if lowercase.ends_with(DOCKERIGNORE_SUFFIX) {
        return false;
    }
    DOCKERFILE_NAMES.iter().any(|base| {
        lowercase == *base
            || lowercase
                .strip_prefix(base)
                .is_some_and(|rest| rest.starts_with('.'))
    }) || lowercase.ends_with(DOCKERFILE_SUFFIX)
}

fn is_compose(lowercase: &str) -> bool {
    let docker_compose = lowercase
        .strip_prefix(DOCKER_COMPOSE_STEM)
        .is_some_and(|rest| rest.starts_with(COMPOSE_VARIANT_SEPARATORS));
    let compose = lowercase
        .strip_prefix(COMPOSE_STEM)
        .is_some_and(|rest| rest.starts_with('.'));
    docker_compose || compose
}

pub fn analyze(
    parser: &mut Parser,
    language: Language,
    source: &[u8],
) -> Option<Result<Vec<Violation>, ParseFailed>> {
    let text = || String::from_utf8_lossy(source);
    match language {
        Language::Sql => Some(Ok(sql::violations(&text(), FIRST_LINE))),
        Language::Dockerfile => Some(Ok(dockerfile::violations(&text()))),
        Language::Compose => Some(
            parser
                .parse(source, None)
                .ok_or(ParseFailed)
                .map(|tree| compose::violations(tree.root_node(), source)),
        ),
        Language::Go
        | Language::Python
        | Language::Java
        | Language::TypeScript
        | Language::Tsx
        | Language::JavaScript
        | Language::Rust
        | Language::Shell => None,
    }
}

const MIGRATION_PREFERENCES: [MigrationRule; 7] = [
    MigrationRule::PreferBigintOverInt,
    MigrationRule::PreferBigintOverSmallint,
    MigrationRule::PreferIdentity,
    MigrationRule::PreferTextField,
    MigrationRule::PreferTimestampTz,
    MigrationRule::PreferRobustStmts,
    MigrationRule::BanCharField,
];
const MIGRATION_MINIMAL_OFF: [MigrationRule; 9] = [
    MigrationRule::PreferBigintOverInt,
    MigrationRule::PreferBigintOverSmallint,
    MigrationRule::PreferIdentity,
    MigrationRule::PreferTextField,
    MigrationRule::PreferTimestampTz,
    MigrationRule::PreferRobustStmts,
    MigrationRule::BanCharField,
    MigrationRule::RequireLockTimeout,
    MigrationRule::RequireStatementTimeout,
];

pub fn relaxed_by(preset: Preset) -> Vec<RuleId> {
    let relaxed: &[Severity] = match preset {
        Preset::Strict => &[],
        Preset::Recommended => &[Severity::Info],
        Preset::Minimal => &[Severity::Info, Severity::Warning],
    };
    let compose: &[ComposeRule] = if preset == Preset::Minimal {
        &ComposeRule::ALL
    } else {
        &[]
    };
    let migrations: &[MigrationRule] = match preset {
        Preset::Strict => &[],
        Preset::Recommended => &MIGRATION_PREFERENCES,
        Preset::Minimal => &MIGRATION_MINIMAL_OFF,
    };
    DockerfileRule::ALL
        .into_iter()
        .filter(|rule| relaxed.contains(&severity(*rule)))
        .map(RuleId::Dockerfile)
        .chain(compose.iter().copied().map(RuleId::Compose))
        .chain(migrations.iter().copied().map(RuleId::Migration))
        .collect()
}

pub fn alembic_violations(root: Node<'_>, source: &[u8]) -> Vec<Violation> {
    alembic::violations(root, source)
}

pub fn violation_diagnostics<'analysis>(
    file: &'analysis FileIdentity,
    violations: &'analysis [Violation],
    limits: Limits,
) -> impl Iterator<Item = Diagnostic> + 'analysis {
    let mut occurrences: HashMap<(RuleId, &str), usize> = HashMap::new();
    violations
        .iter()
        .filter(move |violation| limits.get(violation.rule).is_some())
        .map(move |violation| {
            let occurrence = occurrences
                .entry((violation.rule, violation.subject.as_str()))
                .or_default();
            *occurrence = occurrence.saturating_add(1);
            Diagnostic {
                path: file.display.clone(),
                line: violation.line,
                column: violation.column,
                end_line: violation.line,
                rule: violation.rule,
                message: Message::Violation {
                    rule: violation.rule,
                    subject: violation.subject.clone(),
                },
                value: SINGLE_VIOLATION,
                limit: NO_VIOLATIONS,
                fingerprint: format!(
                    "{}:{}:{}#{occurrence}",
                    violation.rule.as_str(),
                    file.relative,
                    violation.subject
                ),
                relative_path: file.relative.clone(),
                absolute_path: file.absolute.clone(),
                fix: violation.fix.clone(),
            }
        })
}

#[cfg(test)]
mod tests;
