use super::MemoryLoader;
use super::rendering::{APP, assert_rendered};
use super::{SITES_INITIAL, sql};

const PRICES: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="Price",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("tags", models.JSONField(db_default=[])),
            ],
        ),
    ]
"#;

const TYPED_AND_NOW: &str = r#"from django.db import migrations, models
from django.db.models.functions import Now


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [
        migrations.AlterField(
            model_name="price",
            name="tags",
            field=models.JSONField(db_default=models.Value([], output_field=models.JSONField())),
        ),
        migrations.AddField(model_name="price", name="created", field=models.DateTimeField(db_default=Now())),
    ]
"#;

#[test]
fn typed_values_reset_the_default_and_now_reads_the_statement_clock() {
    let loader = MemoryLoader::default().with(APP, "0001_initial", PRICES);
    assert_rendered(
        &sql(&loader, APP, "0002_typed", TYPED_AND_NOW),
        &[
            r#"ALTER COLUMN "tags" SET DEFAULT"#,
            r#""created" timestamp with time zone DEFAULT (STATEMENT_TIMESTAMP()) NOT NULL"#,
        ],
        &[],
    );
}

const LOCAL_FACTORY: &str = r#"from django.db import migrations, models


LEGACY_INDEX = "library_price_legacy"


def drop_index(name):
    return migrations.RunSQL(f'DROP INDEX CONCURRENTLY IF EXISTS "{name}"', reverse_sql=migrations.RunSQL.noop)


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [drop_index("library_price_tags"), drop_index(LEGACY_INDEX)]
"#;

#[test]
fn operations_built_by_a_local_function_bind_its_arguments() {
    let loader = MemoryLoader::default().with(APP, "0001_initial", PRICES);
    assert_rendered(
        &sql(&loader, APP, "0002_factory", LOCAL_FACTORY),
        &[
            r#"DROP INDEX CONCURRENTLY IF EXISTS "library_price_tags""#,
            r#"DROP INDEX CONCURRENTLY IF EXISTS "library_price_legacy""#,
        ],
        &[],
    );
}

const STARRED: &str = r#"import django.db.models.constraints
from django.db import migrations, models
from django.db.models.functions import Now


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(
            name="Starred",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("position", models.IntegerField()),
                ("avatar", models.CharField(db_default=None, default=None, max_length=120, null=True)),
                ("updated", models.DateTimeField(db_default=Now())),
            ],
            options={
                "constraints": [models.UniqueConstraint(fields=("position",), name="starred_position_uniq")],
            },
        ),
        migrations.AddConstraint(
            model_name="starred",
            constraint=models.UniqueConstraint(
                deferrable=django.db.models.constraints.Deferrable["DEFERRED"],
                fields=("position", "avatar"),
                name="starred_position_avatar_uniq",
            ),
        ),
    ]
"#;

#[test]
fn null_and_now_defaults_keep_constraints_inline_and_deferrable_ones_say_so() {
    let loader = MemoryLoader::default();
    assert_rendered(
        &sql(&loader, APP, "0001_initial", STARRED),
        &[
            r#""avatar" varchar(120) DEFAULT NULL NULL"#,
            r#"CONSTRAINT "starred_position_uniq" UNIQUE ("position"))"#,
            r#"ADD CONSTRAINT "starred_position_avatar_uniq" UNIQUE ("position", "avatar") DEFERRABLE INITIALLY DEFERRED"#,
        ],
        &[],
    );
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
