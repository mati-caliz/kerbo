use super::{MemoryLoader, found};
use crate::rules::MigrationRule;

const APP: &str = "library";

const LIBRARY_INITIAL: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="Tag",
            fields=[("id", models.BigAutoField(primary_key=True)), ("label", models.TextField())],
        ),
        migrations.CreateModel(
            name="Author",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("name", models.TextField()),
                ("tags", models.ManyToManyField(to="library.tag")),
            ],
        ),
        migrations.CreateModel(
            name="Book",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("title", models.TextField()),
                ("summary", models.TextField(null=True)),
                ("isbn", models.CharField(max_length=13, db_index=True)),
                ("author", models.ForeignKey(on_delete=models.CASCADE, to="library.author")),
            ],
            options={
                "unique_together": {("title", "author")},
                "indexes": [models.Index(fields=["-title"], name="book_title_idx")],
                "constraints": [
                    models.UniqueConstraint(fields=["isbn"], name="book_isbn_unique"),
                    models.UniqueConstraint(fields=["title"], condition=models.Q(summary=None), name="book_title_partial"),
                    models.CheckConstraint(check=models.Q(id__gt=0), name="book_positive_id"),
                ],
            },
        ),
    ]
"#;

const RENAMES: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [
        migrations.RenameModel(old_name="Author", new_name="Writer"),
        migrations.AlterModelTable(name="book", table="library_volume"),
        migrations.RenameField(model_name="book", old_name="summary", new_name="abstract"),
        migrations.RenameIndex(model_name="book", new_name="volume_title_idx", old_name="book_title_idx"),
    ]
"#;

const FIELDS_AND_META: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [
        migrations.AddField(model_name="book", name="pages", field=models.IntegerField(default=0)),
        migrations.AddField(model_name="book", name="readers", field=models.ManyToManyField(to="library.author")),
        migrations.AlterUniqueTogether(name="book", unique_together={("title", "isbn")}),
        migrations.AddIndex(model_name="book", index=models.Index(fields=["isbn"], name="book_isbn_idx")),
        migrations.AddIndexConcurrently(model_name="book", index=models.Index(models.F("title"), name="book_expr_idx")),
        migrations.RemoveIndex(model_name="book", name="book_title_idx"),
        migrations.RemoveIndexConcurrently(model_name="book", name="book_isbn_idx"),
        migrations.AddConstraint(model_name="book", constraint=models.UniqueConstraint(fields=["title", "isbn"], name="book_pair")),
        migrations.AddConstraint(model_name="book", constraint=models.UniqueConstraint(fields=["isbn"], include=["title"], name="book_cover")),
        migrations.AddConstraint(model_name="book", constraint=models.CheckConstraint(check=models.Q(id__lt=10), name="book_small")),
        migrations.RemoveConstraint(model_name="book", name="book_isbn_unique"),
        migrations.RemoveConstraint(model_name="book", name="book_title_partial"),
        migrations.RemoveField(model_name="book", name="author"),
        migrations.RemoveField(model_name="author", name="tags"),
    ]
"#;

const DROPS: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    atomic = False

    operations = [
        migrations.RunSQL("DROP TABLE legacy", reverse_sql=migrations.RunSQL.noop),
        migrations.RunSQL(migrations.RunSQL.noop),
        migrations.SeparateDatabaseAndState(
            database_operations=[migrations.DeleteModel(name="Tag")],
            state_operations=[migrations.DeleteModel(name="Tag")],
        ),
        migrations.AlterModelOptions(name="book", options={"managed": False}),
        migrations.DeleteModel(name="Book"),
        migrations.DeleteModel(name="Author"),
    ]
"#;

const TAGS_ONLY: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(name="Tag", fields=[("id", models.BigAutoField(primary_key=True))]),
    ]
"#;

const STATE_FROM_VARIABLES: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    state_operations = [
        migrations.CreateModel(name="Tag", fields=[("id", models.BigAutoField(primary_key=True))]),
    ]

    operations = [
        migrations.SeparateDatabaseAndState(state_operations=state_operations),
    ]
"#;

const ADD_FIELD_TO_TAG: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [
        migrations.AddField(model_name="tag", name="label", field=models.CharField(max_length=20, null=True)),
    ]
"#;

const LONG_MODEL_LENGTH: usize = 70;

fn rules(loader: &MemoryLoader, stem: &str, source: &str) -> Vec<MigrationRule> {
    found(loader, APP, stem, source)
        .into_iter()
        .map(|(rule, _)| rule)
        .collect()
}

fn initial() -> MemoryLoader {
    MemoryLoader::default().with(APP, "0001_initial", LIBRARY_INITIAL)
}

fn assert_all(found_rules: &[MigrationRule], expected: &[MigrationRule]) {
    for rule in expected {
        assert!(found_rules.contains(rule), "falta {rule:?} en {found_rules:?}");
    }
}

#[test]
fn create_model_renders_through_tables_indexes_and_constraints() {
    let created = rules(&MemoryLoader::default(), "0001_initial", LIBRARY_INITIAL);
    assert_all(
        &created,
        &[MigrationRule::RequireLockTimeout, MigrationRule::PreferTextField],
    );
}

#[test]
fn renames_flag_tables_and_columns() {
    let renamed = rules(&initial(), "0002_renames", RENAMES);
    assert_all(
        &renamed,
        &[MigrationRule::RenamingTable, MigrationRule::RenamingColumn],
    );
}

#[test]
fn field_and_meta_operations_add_and_drop_objects() {
    let changed = rules(&initial(), "0002_fields_and_meta", FIELDS_AND_META);
    assert_all(
        &changed,
        &[
            MigrationRule::BanDropColumn,
            MigrationRule::BanDropConstraint,
            MigrationRule::DisallowedUniqueConstraint,
            MigrationRule::RequireConcurrentIndexCreation,
            MigrationRule::RequireConcurrentIndexDeletion,
            MigrationRule::BanDropTable,
        ],
    );
}

#[test]
fn drops_and_raw_sql_flag_dropped_tables() {
    let dropped = rules(&initial(), "0002_drops", DROPS);
    assert_all(&dropped, &[MigrationRule::BanDropTable]);
}

#[test]
fn long_names_are_truncated_like_django() {
    let long_model = "A".repeat(LONG_MODEL_LENGTH);
    let source = format!(
        "from django.db import migrations, models\n\n\nclass Migration(migrations.Migration):\n    dependencies = [(\"library\", \"0001_tags\")]\n    operations = [\n        migrations.CreateModel(\n            name=\"{long_model}\",\n            fields=[\n                (\"id\", models.BigAutoField(primary_key=True)),\n                (\"code\", models.CharField(max_length=10, db_index=True)),\n                (\"tags\", models.ManyToManyField(to=\"library.tag\")),\n            ],\n        ),\n    ]\n"
    );
    let loader = MemoryLoader::default().with(APP, "0001_tags", TAGS_ONLY);
    let long_rules = rules(&loader, "0002_long", &source);
    assert_all(
        &long_rules,
        &[MigrationRule::PreferTextField, MigrationRule::IdentifierTooLong],
    );
}

#[test]
fn state_operations_given_as_a_class_variable_build_the_model() {
    let loader = MemoryLoader::default().with(APP, "0001_initial", STATE_FROM_VARIABLES);
    let added = rules(&loader, "0002_label", ADD_FIELD_TO_TAG);
    assert_all(
        &added,
        &[
            MigrationRule::RequireLockTimeout,
            MigrationRule::RequireStatementTimeout,
        ],
    );
}

const SWAPPABLE_USER: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="User",
            fields=[("id", models.AutoField(primary_key=True)), ("email", models.CharField(max_length=254))],
            options={"swappable": "AUTH_USER_MODEL"},
        ),
        migrations.AlterField(model_name="user", name="email", field=models.CharField(max_length=320)),
    ]
"#;

#[test]
fn a_model_swapped_out_by_the_settings_emits_no_sql() {
    let swapped = super::sql(
        &MemoryLoader::default(),
        "account",
        "0001_initial",
        SWAPPABLE_USER,
    );
    assert!(!swapped.contains("CREATE TABLE"), "{swapped}");
    assert!(!swapped.contains("ALTER TABLE"), "{swapped}");
    let active = super::sql(&MemoryLoader::default(), "auth", "0001_initial", SWAPPABLE_USER);
    assert!(active.contains(r#"CREATE TABLE "auth_user""#), "{active}");
}
