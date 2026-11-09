use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tree_sitter::{Parser, Tree};

use super::dispatch;
use super::history::{Loader, Location, location, state_before};
use super::operations::Migration;
use super::parse::{Reader, migration};
use super::render;
use super::settings::Settings;
use crate::language::Language;
use crate::rules::{MigrationRule, RuleId};

mod dump;
mod expressions;
mod files;
mod graph;
mod operations;
mod rendering;

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
                settings: &Settings::default(),
                apps_root: None,
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
    fn apps(&self) -> Vec<String> {
        self.apps.keys().cloned().collect()
    }

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
    dependencies = [("sites", "0001_initial")]

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
    dependencies = [("blog", "0001_initial")]

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
            (MigrationRule::PreferBigintOverInt, 8),
            (MigrationRule::RequireLockTimeout, 8),
            (MigrationRule::RequireStatementTimeout, 8),
        ]
    );
}

#[test]
fn replayed_history_drives_alter_and_remove_field() {
    let loader = MemoryLoader::default()
        .with("sites", "0001_initial", SITES_INITIAL)
        .with("blog", "0001_initial", ARTICLES);
    let rules = found(&loader, "blog", "0002_required_body", ARTICLES_REQUIRED_BODY);
    assert!(rules.contains(&(MigrationRule::BanDropColumn, 13)), "{rules:?}");
    assert!(
        rules.contains(&(MigrationRule::RequireLockTimeout, 8)),
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

const SITES_THROUGH_NAMES: &str = r#"from django.db import migrations, models

CREATE_SITE = migrations.CreateModel(
    name="Site",
    fields=[("id", models.AutoField(primary_key=True)), ("domain", models.CharField(max_length=100))],
)


class Migration(migrations.Migration):
    state_operations = [CREATE_SITE]
    alias = state_operations
    operations = alias + [migrations.RemoveField(model_name="site", name="domain")]
"#;

#[test]
fn operations_bound_to_names_and_concatenated_are_followed() {
    let rules = found(
        &MemoryLoader::default(),
        "sites",
        "0001_initial",
        SITES_THROUGH_NAMES,
    );
    assert_eq!(
        rules,
        vec![
            (MigrationRule::BanDropColumn, 12),
            (MigrationRule::PreferBigintOverInt, 3),
            (MigrationRule::PreferTextField, 3),
            (MigrationRule::RequireLockTimeout, 12),
            (MigrationRule::RequireStatementTimeout, 12),
        ]
    );
}

fn sql(loader: &MemoryLoader, app: &str, stem: &str, source: &str) -> String {
    let tree = parsed(source);
    let parsed_migration = migration(
        tree.root_node(),
        Reader {
            source: source.as_bytes(),
            app,
            settings: &Settings::default(),
            apps_root: None,
        },
    )
    .expect("es una migración");
    let current = Location {
        app: app.to_owned(),
        stem: stem.to_owned(),
        apps_root: PathBuf::new(),
    };
    let mut state = state_before(loader, &current, &parsed_migration);
    dispatch::sql(&parsed_migration, app, &mut state)
}

const TYPED_DEFAULTS: &str = r#"from datetime import timedelta
from decimal import Decimal
import os
import uuid

from django.contrib.postgres.fields import ArrayField
from django.db import migrations, models
from django.utils import timezone


class Migration(migrations.Migration):
    dependencies = [("sites", "0001_initial")]

    operations = [
        migrations.AddField(model_name="site", name="metadata", field=models.JSONField(default=dict)),
        migrations.AddField(model_name="site", name="tags", field=models.JSONField(default=[])),
        migrations.AddField(model_name="site", name="token", field=models.UUIDField(default=uuid.uuid4)),
        migrations.AddField(model_name="site", name="created", field=models.DateTimeField(default=timezone.now)),
        migrations.AddField(
            model_name="site",
            name="balance",
            field=models.DecimalField(max_digits=12, decimal_places=3, default=Decimal("0.0")),
        ),
        migrations.AddField(model_name="site", name="grace", field=models.DurationField(default=timedelta(days=1, minutes=30))),
        migrations.AddField(model_name="site", name="limit", field=models.IntegerField(default=Decimal(5))),
        migrations.AddField(model_name="site", name="region", field=models.TextField(default=os.environ.get("REGION", "ar"))),
        migrations.AddField(model_name="site", name="codes", field=ArrayField(models.TextField(), size=3, default=list)),
        migrations.AddField(model_name="site", name="extra", field=models.JSONField(db_default={})),
    ]
"#;

#[test]
fn defaults_are_rendered_with_the_type_postgres_receives() {
    let loader = MemoryLoader::default().with("sites", "0001_initial", SITES_INITIAL);
    let rendered = sql(&loader, "sites", "0002_typed_defaults", TYPED_DEFAULTS);
    for expected in [
        "\"metadata\" jsonb DEFAULT '{}'::jsonb NOT NULL",
        "\"tags\" jsonb DEFAULT '[]'::jsonb NOT NULL",
        "\"token\" uuid DEFAULT '00000000000000000000000000000000'::uuid NOT NULL",
        "\"created\" timestamp with time zone DEFAULT '2000-01-01 00:00:00+00:00'::timestamptz NOT NULL",
        "\"balance\" numeric(12, 3) DEFAULT 0.0 NOT NULL",
        "\"grace\" interval DEFAULT '1 day 0:30:00'::interval NOT NULL",
        "\"limit\" integer DEFAULT 5 NOT NULL",
        "\"region\" text DEFAULT 'ar' NOT NULL",
        "\"codes\" text[3] DEFAULT '{}' NOT NULL",
        "\"extra\" jsonb DEFAULT '{}'::jsonb NOT NULL",
    ] {
        assert!(rendered.contains(expected), "falta {expected} en {rendered}");
    }
}

const CONDITIONS: &str = r#"from decimal import Decimal

from django.contrib.postgres.operations import CreateExtension, TrigramExtension
from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("sites", "0001_initial")]

    operations = [
        TrigramExtension(),
        CreateExtension("citext"),
        migrations.AddField(model_name="site", name="active", field=models.BooleanField(default=True)),
        migrations.AddField(model_name="site", name="rank", field=models.IntegerField(null=True)),
        migrations.AddField(
            model_name="site",
            name="parent",
            field=models.ForeignKey(null=True, on_delete=models.CASCADE, to="sites.site"),
        ),
        migrations.AddConstraint(
            model_name="site",
            constraint=models.CheckConstraint(
                condition=models.Q(rank__gte=0) & models.Q(rank__lte=Decimal("10")), name="rank_range"
            ),
        ),
        migrations.AddConstraint(
            model_name="site",
            constraint=models.UniqueConstraint(fields=["domain"], condition=~models.Q(active=True), name="inactive_domain"),
        ),
        migrations.AddConstraint(
            model_name="site",
            constraint=models.UniqueConstraint(
                fields=["name"], condition=models.Q(rank__in=[1, 2, 2, None]) | models.Q(parent=None), name="ranked_name"
            ),
        ),
        migrations.AddConstraint(
            model_name="site",
            constraint=models.CheckConstraint(check=~models.Q(rank=5), name="not_five"),
        ),
        migrations.AddConstraint(
            model_name="site",
            constraint=models.UniqueConstraint(fields=["domain", "name"], nulls_distinct=False, name="pair"),
        ),
        migrations.AddConstraint(
            model_name="site",
            constraint=models.CheckConstraint(check=models.Q(domain__startswith="www"), name="opaque_lookup"),
        ),
    ]
"#;

#[test]
fn conditions_extensions_and_nulls_distinct_render_like_django() {
    let loader = MemoryLoader::default().with("sites", "0001_initial", SITES_INITIAL);
    let rendered = sql(&loader, "sites", "0002_conditions", CONDITIONS);
    for expected in [
        "CREATE EXTENSION IF NOT EXISTS \"pg_trgm\"",
        "CREATE EXTENSION IF NOT EXISTS \"citext\"",
        "ADD CONSTRAINT \"rank_range\" CHECK ((\"rank\" >= 0 AND \"rank\" <= 10))",
        "CREATE UNIQUE INDEX \"inactive_domain\" ON \"django_site\" (\"domain\") WHERE NOT (\"active\")",
        "CREATE UNIQUE INDEX \"ranked_name\" ON \"django_site\" (\"name\") WHERE (\"rank\" IN (1, 2) OR \"parent_id\" IS NULL)",
        "ADD CONSTRAINT \"not_five\" CHECK (NOT (\"rank\" = 5 AND \"rank\" IS NOT NULL))",
        "ADD CONSTRAINT \"pair\" UNIQUE NULLS NOT DISTINCT (\"domain\", \"name\")",
        "ADD CONSTRAINT \"opaque_lookup\" CHECK (true)",
    ] {
        assert!(rendered.contains(expected), "falta {expected} en {rendered}");
    }
}
