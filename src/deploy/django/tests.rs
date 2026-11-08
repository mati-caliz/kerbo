use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tree_sitter::{Parser, Tree};

use super::history::{Loader, Location, location};
use super::operations::Migration;
use super::parse::{Reader, migration};
use super::render;
use crate::language::Language;
use crate::rules::{MigrationRule, RuleId};

mod files;
mod operations;

#[derive(Debug, Default)]
struct MemoryLoader {
    apps: HashMap<String, Vec<(String, Arc<Migration>)>>,
}

impl MemoryLoader {
    fn with(mut self, app: &str, stem: &str, source: &str) -> Self {
        let tree = parsed(source);
        let parsed_migration = migration(
            tree.root_node(),
            Reader {
                source: source.as_bytes(),
                app,
                variables: &[],
            },
        )
        .expect("es una migración");
        let entries = self.apps.entry(app.to_owned()).or_default();
        entries.push((stem.to_owned(), Arc::new(parsed_migration)));
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        self
    }
}

impl Loader for MemoryLoader {
    fn migrations(&self, app: &str) -> Vec<(String, Arc<Migration>)> {
        self.apps.get(app).cloned().unwrap_or_default()
    }
}

fn parsed(source: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    parser.parse(source, None).expect("se parsea")
}

fn found(loader: &MemoryLoader, app: &str, stem: &str, source: &str) -> Vec<(MigrationRule, usize)> {
    let tree = parsed(source);
    let current = Location {
        app: app.to_owned(),
        stem: stem.to_owned(),
        apps_root: PathBuf::new(),
    };
    let mut rules: Vec<(MigrationRule, usize)> =
        render(tree.root_node(), source.as_bytes(), &current, loader)
            .into_iter()
            .map(|violation| {
                let RuleId::Migration(rule) = violation.rule else {
                    panic!("regla inesperada {:?}", violation.rule);
                };
                (rule, violation.line)
            })
            .collect();
    rules.sort_by_key(|(rule, line)| (rule.as_str(), *line));
    rules
}

const SITES_INITIAL: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="Site",
            fields=[
                ("id", models.AutoField(verbose_name="ID", serialize=False, auto_created=True, primary_key=True)),
                ("domain", models.CharField(max_length=100, verbose_name="domain name")),
                ("name", models.CharField(max_length=50, verbose_name="display name")),
            ],
            options={"db_table": "django_site"},
        ),
    ]
"#;

const SITES_UNIQUE_DOMAIN: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("sites", "0001_initial")]

    operations = [
        migrations.AlterField(
            model_name="site",
            name="domain",
            field=models.CharField(max_length=100, unique=True, verbose_name="domain name"),
        ),
    ]
"#;

#[test]
fn create_model_flags_column_types_at_the_operation_call() {
    let rules = found(&MemoryLoader::default(), "sites", "0001_initial", SITES_INITIAL);
    assert_eq!(
        rules,
        vec![
            (MigrationRule::PreferBigintOverInt, 6),
            (MigrationRule::PreferTextField, 6),
            (MigrationRule::PreferTextField, 6),
        ]
    );
}

#[test]
fn altering_to_unique_adds_constraint_and_like_index() {
    let loader = MemoryLoader::default().with("sites", "0001_initial", SITES_INITIAL);
    let rules: Vec<MigrationRule> = found(&loader, "sites", "0002_alter_domain_unique", SITES_UNIQUE_DOMAIN)
        .into_iter()
        .map(|(rule, _)| rule)
        .collect();
    assert_eq!(
        rules,
        vec![
            MigrationRule::ConstraintMissingNotValid,
            MigrationRule::DisallowedUniqueConstraint,
            MigrationRule::RequireConcurrentIndexCreation,
            MigrationRule::RequireLockTimeout,
            MigrationRule::RequireStatementTimeout,
        ]
    );
}

const ARTICLES: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="Article",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("site", models.ForeignKey(on_delete=models.CASCADE, to="sites.site")),
                ("body", models.TextField(null=True)),
            ],
        ),
    ]
"#;

const ARTICLES_REQUIRED_BODY: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.AlterField(
            model_name="article",
            name="body",
            field=models.TextField(default="vacío"),
        ),
        migrations.RemoveField(model_name="article", name="site"),
    ]
"#;

#[test]
fn foreign_keys_load_the_related_app_and_reference_its_key_type() {
    let loader = MemoryLoader::default().with("sites", "0001_initial", SITES_INITIAL);
    let rules = found(&loader, "blog", "0001_initial", ARTICLES);
    assert_eq!(
        rules,
        vec![
            (MigrationRule::PreferBigintOverInt, 6),
            (MigrationRule::RequireLockTimeout, 6),
            (MigrationRule::RequireStatementTimeout, 6),
        ]
    );
}

#[test]
fn replayed_history_drives_alter_and_remove_field() {
    let loader = MemoryLoader::default()
        .with("sites", "0001_initial", SITES_INITIAL)
        .with("blog", "0001_initial", ARTICLES);
    let rules = found(&loader, "blog", "0002_required_body", ARTICLES_REQUIRED_BODY);
    assert!(rules.contains(&(MigrationRule::BanDropColumn, 11)), "{rules:?}");
    assert!(
        rules.contains(&(MigrationRule::RequireLockTimeout, 6)),
        "{rules:?}"
    );
}

#[test]
fn location_reads_the_app_from_the_migrations_directory() {
    let path = Path::new("proyecto")
        .join("blog")
        .join("migrations")
        .join("0002_body.py");
    let found_location = location(&path).expect("es una migración de Django");
    assert_eq!(found_location.app, "blog");
    assert_eq!(found_location.stem, "0002_body");
    assert_eq!(found_location.apps_root, PathBuf::from("proyecto"));
    assert!(location(&Path::new("blog").join("migrations").join("__init__.py")).is_none());
}
