use super::{MemoryLoader, sql};

const APP: &str = "chat";

const EXPRESSION_INDEXES: &str = r#"from enum import Enum

import django.db.models.functions.text
from django.contrib.postgres.indexes import GinIndex
from django.db import migrations, models


class Policy(Enum):
    open_topics = 2


class Migration(migrations.Migration):
    ARCHIVED = 4

    operations = [
        migrations.CreateModel(
            name="Message",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("subject", models.CharField(max_length=60)),
                ("flags", models.BitField(default=0)),
                ("status", models.IntegerField(default=ARCHIVED)),
                ("policy", models.IntegerField(default=Policy.open_topics.value)),
                ("search", models.TextField()),
            ],
            options={
                "indexes": [
                    models.Index(models.OrderBy(models.F("id"), descending=True, nulls_last=True), name="msg_id_desc"),
                    models.Index(models.F("id").desc(nulls_last=True), name="msg_id_method"),
                    models.Index(django.db.models.functions.text.Upper("subject"), "id", name="msg_upper"),
                    GinIndex(models.F("search"), fastupdate=False, name="msg_search"),
                    models.Index(
                        "id",
                        condition=models.Q(("flags__andnz", 8), ("flags__andnz", 16), _connector="OR"),
                        name="msg_flagged",
                    ),
                    models.Index("id", condition=models.Q(flags__andz=2), name="msg_unread"),
                    models.Index("id", condition=models.Q(status=ARCHIVED), name="msg_archived"),
                ],
                "constraints": [
                    models.UniqueConstraint(models.functions.Lower("subject"), name="msg_subject_uniq"),
                ],
            },
        ),
    ]
"#;

fn rendered() -> String {
    sql(&MemoryLoader::default(), APP, "0001_initial", EXPRESSION_INDEXES)
}

#[test]
fn ordered_expressions_render_direction_and_nulls() {
    let sql = rendered();
    assert!(
        sql.contains(r#"CREATE INDEX "msg_id_desc" ON "chat_message" ("id" DESC NULLS LAST)"#),
        "{sql}"
    );
    assert!(
        sql.contains(r#"CREATE INDEX "msg_id_method" ON "chat_message" ("id" DESC NULLS LAST)"#),
        "{sql}"
    );
}

#[test]
fn function_expressions_and_positional_names_become_columns() {
    let sql = rendered();
    assert!(
        sql.contains(r#"CREATE INDEX "msg_upper" ON "chat_message" ((UPPER("subject")), "id")"#),
        "{sql}"
    );
    assert!(
        sql.contains(r#"CREATE UNIQUE INDEX "msg_subject_uniq" ON "chat_message" ((LOWER("subject")))"#),
        "{sql}"
    );
}

#[test]
fn gin_storage_parameters_render_after_the_columns() {
    let sql = rendered();
    assert!(
        sql.contains(
            r#"CREATE INDEX "msg_search" ON "chat_message" USING gin ("search") WITH (fastupdate = off)"#
        ),
        "{sql}"
    );
}

#[test]
fn bit_lookups_connectors_and_constants_render_in_conditions() {
    let sql = rendered();
    assert!(
        sql.contains(r#"WHERE ("flags" & 8 != 0 OR "flags" & 16 != 0)"#),
        "{sql}"
    );
    assert!(sql.contains(r#"WHERE "flags" & 2 = 0"#), "{sql}");
    assert!(sql.contains(r#"WHERE "status" = 4"#), "{sql}");
}

#[test]
fn bit_fields_and_constant_defaults_render_as_integers() {
    let sql = rendered();
    assert!(sql.contains(r#""flags" bigint NOT NULL"#), "{sql}");
    assert!(!sql.contains("ARCHIVED"), "{sql}");
}

const CONSTANT_DEFAULTS: &str = r#"from enum import Enum

from django.db import migrations, models


class Policy(Enum):
    open_topics = 2


class Migration(migrations.Migration):
    dependencies = [("chat", "0001_initial")]

    STARTED = 1
    ENABLED = True

    operations = [
        migrations.AddField(model_name="message", name="stage", field=models.IntegerField(default=STARTED)),
        migrations.AddField(
            model_name="message", name="topics", field=models.IntegerField(default=Policy.open_topics.value)
        ),
        migrations.AddField(
            model_name="message", name="level", field=models.IntegerField(default=7 if ENABLED else 9)
        ),
    ]
"#;

#[test]
fn class_enum_and_conditional_defaults_resolve_to_their_value() {
    let loader = MemoryLoader::default().with(APP, "0001_initial", EXPRESSION_INDEXES);
    let sql = sql(&loader, APP, "0002_defaults", CONSTANT_DEFAULTS);
    assert!(
        sql.contains(r#"ADD COLUMN "stage" integer DEFAULT 1 NOT NULL"#),
        "{sql}"
    );
    assert!(
        sql.contains(r#"ADD COLUMN "topics" integer DEFAULT 2 NOT NULL"#),
        "{sql}"
    );
    assert!(
        sql.contains(r#"ADD COLUMN "level" integer DEFAULT 7 NOT NULL"#),
        "{sql}"
    );
}
