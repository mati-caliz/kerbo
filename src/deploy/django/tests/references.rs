use super::rendering::{APP, assert_rendered};
use super::{MemoryLoader, sql};

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

const SETTING_PAGE: &str = r#"import swapper
from django.conf import settings
from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [migrations.swappable_dependency(settings.LIBRARY_PAGE_MODEL)]

    operations = [
        migrations.CreateModel(
            name="Page",
            fields=[("id", models.AutoField(primary_key=True))],
            options={"swappable": swapper.swappable_setting("library", "Page")},
        ),
        migrations.CreateModel(
            name="Site",
            fields=[
                ("id", models.AutoField(primary_key=True)),
                ("root", models.ForeignKey(on_delete=models.CASCADE, to=settings.LIBRARY_PAGE_MODEL)),
            ],
        ),
    ]
"#;

#[test]
fn an_unset_swappable_setting_points_to_the_model_that_declares_it() {
    let loader = MemoryLoader::default();
    assert_rendered(
        &sql(&loader, APP, "0001_initial", SETTING_PAGE),
        &[
            r#""root_id" integer NOT NULL"#,
            r#"REFERENCES "library_page" ("id")"#,
        ],
        &["settings"],
    );
}

const PLAIN_PAGE: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [migrations.CreateModel(name="Page", fields=[("id", models.AutoField(primary_key=True))])]
"#;

const SQUASHED_PAGE: &str = r#"import swapper
from django.db import migrations, models


class Migration(migrations.Migration):
    replaces = [("library", "0001_initial")]

    operations = [
        migrations.CreateModel(
            name="Page",
            fields=[("id", models.AutoField(primary_key=True))],
            options={"swappable": swapper.swappable_setting("library", "Page")},
        ),
    ]
"#;

const SHELF_ON_SETTING_PAGE: &str = r#"from django.conf import settings
from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("library", "0001_initial")]

    operations = [
        migrations.CreateModel(
            name="Shelf",
            fields=[
                ("id", models.AutoField(primary_key=True)),
                ("page", models.ForeignKey(on_delete=models.CASCADE, to=settings.LIBRARY_PAGE_MODEL)),
            ],
        ),
    ]
"#;

#[test]
fn a_swappable_setting_declared_outside_the_ancestors_still_points_to_its_model() {
    let loader = MemoryLoader::default()
        .with(APP, "0001_initial", PLAIN_PAGE)
        .with(APP, "0001_squashed_0002_page", SQUASHED_PAGE);
    assert_rendered(
        &sql(&loader, "shelves", "0001_initial", SHELF_ON_SETTING_PAGE),
        &[
            r#""page_id" integer NOT NULL"#,
            r#"REFERENCES "library_page" ("id")"#,
        ],
        &["bigint", "settings"],
    );
}

const TAGGED_ITEM: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [("taggit", "0001_initial")]

    operations = [
        migrations.CreateModel(
            name="TaggedItem",
            fields=[
                ("id", models.AutoField(primary_key=True)),
                ("tag", models.ForeignKey(on_delete=models.CASCADE, to="taggit.Tag")),
            ],
        ),
    ]
"#;

#[test]
fn taggit_tags_outside_the_project_keep_their_integer_key() {
    assert_rendered(
        &sql(&MemoryLoader::default(), APP, "0001_initial", TAGGED_ITEM),
        &[
            r#""tag_id" integer NOT NULL"#,
            r#"REFERENCES "taggit_tag" ("id")"#,
        ],
        &["bigint"],
    );
}
