use std::path::PathBuf;

use super::{Violation, relaxed_by, violation_diagnostics};
use crate::cache::AnalysisCache;
use crate::config::Config;
use crate::i18n::{Lang, Message};
use crate::language::Stack;
use crate::limits::{Limits, Preset};
use crate::rules::{ComposeRule, DockerfileRule, FileIdentity, MigrationRule, RuleId};
use crate::scan::{ScanRequest, scan};
use crate::test_support::TempTree;

fn file() -> FileIdentity {
    FileIdentity {
        display: "./db/migrations/1.sql".to_owned(),
        relative: "db/migrations/1.sql".to_owned(),
        absolute: PathBuf::from("db/migrations/1.sql"),
    }
}

fn violation(rule: RuleId, subject: &str) -> Violation {
    Violation {
        rule,
        line: 3,
        column: 1,
        subject: subject.to_owned(),
        fix: None,
    }
}

#[test]
fn turns_violations_into_diagnostics_with_stable_fingerprints() {
    let drop = RuleId::Migration(MigrationRule::BanDropColumn);
    let found = [
        violation(drop, "email"),
        violation(drop, "email"),
        violation(drop, ""),
    ];
    let diagnostics: Vec<_> =
        violation_diagnostics(&file(), &found, Limits::default().with_rule(drop, Some(0))).collect();
    let fingerprints: Vec<&str> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.fingerprint.as_str())
        .collect();
    assert_eq!(
        fingerprints,
        [
            "ban-drop-column:db/migrations/1.sql:email#1",
            "ban-drop-column:db/migrations/1.sql:email#2",
            "ban-drop-column:db/migrations/1.sql:#1"
        ]
    );
    let message = Lang::English.message(&diagnostics[0].message);
    assert_eq!(
        message,
        "Drops a column the deployed code may still read: `email`"
    );
    assert_eq!(
        Lang::Spanish.message(&Message::Violation {
            rule: drop,
            subject: String::new()
        }),
        "Borra una columna que el código desplegado todavía puede leer"
    );
    assert_eq!(
        violation_diagnostics(&file(), &found, Limits::default()).count(),
        0
    );
}

#[test]
fn rules_are_named_like_squawk_and_hadolint() {
    assert_eq!(
        "DL3008".parse::<RuleId>().ok(),
        Some(RuleId::Dockerfile(DockerfileRule::Dl3008))
    );
    assert_eq!(
        "require-concurrent-index-creation".parse::<RuleId>().ok(),
        Some(RuleId::Migration(MigrationRule::RequireConcurrentIndexCreation))
    );
    let explained = Lang::Spanish.explanation_text(RuleId::Compose(ComposeRule::HealthcheckStartPeriod));
    assert!(explained.starts_with("compose-healthcheck-start-period: El healthcheck no tiene start_period"));
    let serialized =
        serde_json::to_string(&RuleId::Dockerfile(DockerfileRule::Dl3059)).expect("se serializa");
    assert_eq!(serialized, "\"dl3059\"");
    let parsed: RuleId = serde_json::from_str("\"ban-drop-table\"").expect("se lee");
    assert_eq!(parsed, RuleId::Migration(MigrationRule::BanDropTable));
    assert!(serde_json::from_str::<RuleId>("\"no-existe\"").is_err());
    assert!(serde_json::from_str::<RuleId>("3").is_err());
}

#[test]
fn looser_presets_drop_the_hadolint_levels_they_tolerate() {
    assert_eq!(relaxed_by(Preset::Strict), Vec::new());
    let recommended = relaxed_by(Preset::Recommended);
    assert!(recommended.contains(&RuleId::Dockerfile(DockerfileRule::Dl3059)));
    assert!(!recommended.contains(&RuleId::Dockerfile(DockerfileRule::Dl3008)));
    let minimal = relaxed_by(Preset::Minimal);
    assert!(minimal.contains(&RuleId::Dockerfile(DockerfileRule::Dl3008)));
    assert!(!minimal.contains(&RuleId::Dockerfile(DockerfileRule::Dl3020)));
    assert!(minimal.contains(&RuleId::Compose(ComposeRule::MemoryLimit)));
    let migrations = Preset::Minimal.limits(crate::language::Language::Sql, false);
    assert_eq!(
        migrations.get(RuleId::Migration(MigrationRule::BanDropTable)),
        Some(0)
    );
    assert_eq!(migrations.get(RuleId::FileLength), None);
}

#[test]
fn scans_migrations_dockerfiles_and_compose_files_through_the_cache() {
    let tree = TempTree::new("deploy-scan");
    tree.write(
        "prisma/migrations/1_init/migration.sql",
        "ALTER TABLE users DROP COLUMN email;\n",
    );
    tree.write("api/Dockerfile", "FROM node\n");
    tree.write("compose.yaml", "services:\n  api:\n    build: ./api\n");
    tree.write(
        "alembic/versions/1_drop.py",
        "from alembic import op\n\n\ndef upgrade():\n    op.drop_table('old')\n",
    );
    tree.write("seeds/data.sql", "DROP TABLE users;\n");
    let config = Config::load(None, tree.path()).expect("la configuración es válida");
    let cache = AnalysisCache::open(tree.path().join(".cache"), 1 << 20);
    let roots = [tree.path().to_path_buf()];
    let run = |stacks: &[Stack]| {
        scan(&ScanRequest {
            roots: &roots,
            stacks,
            overrides: &[],
            config: &config,
            cache: Some(&cache),
        })
    };
    let expected = vec![
        ("alembic/versions/1_drop.py".to_owned(), "ban-drop-table"),
        ("alembic/versions/1_drop.py".to_owned(), "require-lock-timeout"),
        (
            "alembic/versions/1_drop.py".to_owned(),
            "require-statement-timeout",
        ),
        ("api/Dockerfile".to_owned(), "dl3006"),
        ("compose.yaml".to_owned(), "compose-memory-limit"),
        ("compose.yaml".to_owned(), "compose-cpu-limit"),
        ("compose.yaml".to_owned(), "compose-restart"),
        (
            "prisma/migrations/1_init/migration.sql".to_owned(),
            "require-lock-timeout",
        ),
        (
            "prisma/migrations/1_init/migration.sql".to_owned(),
            "require-statement-timeout",
        ),
        (
            "prisma/migrations/1_init/migration.sql".to_owned(),
            "ban-drop-column",
        ),
    ];
    for _ in 0..2 {
        let outcome = run(&[]);
        assert_eq!(outcome.files, 4);
        let found: Vec<(String, &str)> = outcome
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.relative_path.clone(), diagnostic.rule.as_str()))
            .collect();
        assert_eq!(found, expected);
    }
    assert_eq!(run(&[Stack::Sql]).diagnostics.len(), 3);
    assert_eq!(run(&[Stack::Docker]).diagnostics.len(), 4);
}
