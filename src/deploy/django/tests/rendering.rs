use super::{MemoryLoader, sql};

fn assert_rendered(rendered: &str, present: &[&str], absent: &[&str]) {
    for expected in present {
        assert!(rendered.contains(expected), "falta {expected} en {rendered}");
    }
    for unexpected in absent {
        assert!(!rendered.contains(unexpected), "sobra {unexpected} en {rendered}");
    }
}

const APP: &str = "library";

const PARENTHESIZED_SLUGS: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = (
        [
            migrations.CreateModel(
                name="Tag",
                fields=[("id", models.BigAutoField(primary_key=True)), ("slug", models.SlugField(max_length=50))],
            ),
        ]
        # comentario entre listas
        + [migrations.AddField(model_name="tag", name="code", field=models.SlugField(db_index=False))]
    )
"#;

#[test]
fn parenthesized_operations_are_followed_and_slugs_get_an_index() {
    let loader = MemoryLoader::default();
    assert_rendered(
        &sql(&loader, APP, "0001_initial", PARENTHESIZED_SLUGS),
        &[
            r#"CREATE TABLE "library_tag""#,
            r#"CREATE INDEX "library_tag_slug_"#,
            "varchar_pattern_ops",
            r#"ADD COLUMN "code""#,
        ],
        &[r#"CREATE INDEX "library_tag_code_"#],
    );
}

const NULLABLE_META: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="Item",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("meta", models.JSONField(null=True, default=dict)),
                ("rank", models.IntegerField(null=True, default=0)),
            ],
        ),
    ]
"#;

const REQUIRED_META: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [
        migrations.AlterField(model_name="item", name="meta", field=models.JSONField(default=dict)),
        migrations.AlterField(model_name="item", name="rank", field=models.IntegerField(default=0)),
    ]
"#;

#[test]
fn json_defaults_never_compare_equal_when_a_column_becomes_required() {
    let loader = MemoryLoader::default().with(APP, "0001_initial", NULLABLE_META);
    assert_rendered(
        &sql(&loader, APP, "0002_required", REQUIRED_META),
        &[
            r#"ALTER COLUMN "meta" SET DEFAULT '{}'::jsonb"#,
            r#"ALTER COLUMN "meta" DROP DEFAULT"#,
            r#"ALTER COLUMN "rank" SET NOT NULL"#,
        ],
        &[r#"ALTER COLUMN "rank" SET DEFAULT"#],
    );
}

const WITH_TAGS: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(name="Tag", fields=[("id", models.BigAutoField(primary_key=True))]),
        migrations.CreateModel(
            name="Author",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("tags", models.ManyToManyField(to="library.tag")),
            ],
        ),
    ]
"#;

const RENAME_BOTH_ENDS: &str = r#"from django.db import migrations


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [
        migrations.RenameModel(old_name="Author", new_name="Writer"),
        migrations.RenameModel(old_name="Tag", new_name="Label"),
    ]
"#;

#[test]
fn renaming_a_model_renames_its_many_to_many_tables_and_columns() {
    let loader = MemoryLoader::default().with(APP, "0001_initial", WITH_TAGS);
    assert_rendered(
        &sql(&loader, APP, "0002_renames", RENAME_BOTH_ENDS),
        &[
            r#"ALTER TABLE "library_author" RENAME TO "library_writer""#,
            r#"ALTER TABLE "library_author_tags" RENAME TO "library_writer_tags""#,
            r#"ALTER TABLE "library_writer_tags" RENAME COLUMN "author_id" TO "writer_id""#,
            r#"ALTER TABLE "library_tag" RENAME TO "library_label""#,
            r#"ALTER TABLE "library_writer_tags" RENAME COLUMN "tag_id" TO "label_id""#,
        ],
        &[],
    );
}

const DECIMAL_DATABASE_DEFAULT: &str = r#"from decimal import Decimal

from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="Price",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("amount", models.DecimalField(max_digits=12, decimal_places=3, db_default=Decimal(0))),
            ],
        ),
    ]
"#;

#[test]
fn decimal_database_defaults_render_their_number() {
    let loader = MemoryLoader::default();
    assert_rendered(
        &sql(&loader, APP, "0001_initial", DECIMAL_DATABASE_DEFAULT),
        &[r#""amount" numeric(12, 3) DEFAULT 0 NOT NULL"#],
        &[],
    );
}

const POSITIVE_ORDER: &str = r#"from django.contrib.postgres.fields import ArrayField
from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="Image",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("order", models.PositiveIntegerField(editable=False)),
                ("actions", ArrayField(base_field=models.CharField(max_length=128), size=None)),
            ],
        ),
    ]
"#;

const RENAMED_ORDER: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [
        migrations.RenameField(model_name="image", old_name="order", new_name="sort_order"),
        migrations.AlterField(
            model_name="image",
            name="sort_order",
            field=models.PositiveIntegerField(db_index=True, editable=False),
        ),
    ]
"#;

#[test]
fn renaming_a_positive_field_keeps_its_check_and_unsized_arrays_have_no_size() {
    let loader = MemoryLoader::default().with(APP, "0001_initial", POSITIVE_ORDER);
    assert_rendered(
        &sql(&loader, APP, "0001_initial", POSITIVE_ORDER),
        &[r#""actions" varchar(128)[] NOT NULL"#],
        &["[None]"],
    );
    assert_rendered(
        &sql(&loader, APP, "0002_sort_order", RENAMED_ORDER),
        &[
            r#"RENAME COLUMN "order" TO "sort_order""#,
            r#"CREATE INDEX "library_image_sort_order_"#,
        ],
        &["CHECK"],
    );
}

const TABLE_MOVED_BY_PYTHON: &str = r#"from django.db import migrations, models


def rename_tables(apps, schema_editor):
    Tag = apps.get_model("library", "Tag")
    schema_editor.alter_db_table(Tag, "library_tag", "catalog_tag")
    schema_editor.alter_db_table(Tag, "library_same", "library_same")


def forget_tables(apps, schema_editor):
    schema_editor.alter_db_table(None, "library_note", "catalog_note")


class Migration(migrations.Migration):
    operations = [
        migrations.RunPython(forget_tables, migrations.RunPython.noop),
        migrations.SeparateDatabaseAndState(
            database_operations=[migrations.RunPython(rename_tables, migrations.RunPython.noop)],
            state_operations=[],
        ),
    ]
"#;

#[test]
fn schema_editor_renames_inside_separate_database_and_state_are_rendered() {
    let loader = MemoryLoader::default();
    assert_rendered(
        &sql(&loader, APP, "0001_initial", TABLE_MOVED_BY_PYTHON),
        &[r#"ALTER TABLE "library_tag" RENAME TO "catalog_tag""#],
        &["library_note", "library_same"],
    );
}

const SWAPPABLE_PAGE: &str = r#"import swapper
from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(name="Page", fields=[("id", models.AutoField(primary_key=True))]),
        migrations.CreateModel(
            name="Site",
            fields=[
                ("id", models.AutoField(primary_key=True)),
                ("root", models.ForeignKey(on_delete=models.CASCADE, to=swapper.get_model_name("library", "Page"))),
            ],
        ),
    ]
"#;

#[test]
fn a_swapper_reference_points_to_the_default_model() {
    let loader = MemoryLoader::default();
    assert_rendered(
        &sql(&loader, APP, "0001_initial", SWAPPABLE_PAGE),
        &[
            r#""root_id" integer NOT NULL"#,
            r#"REFERENCES "library_page" ("id")"#,
        ],
        &["text_pattern_ops"],
    );
}
