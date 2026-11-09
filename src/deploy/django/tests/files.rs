use std::path::Path;

use super::parsed;
use crate::deploy::django::history::location;
use crate::deploy::django::{fingerprint, rendered_sql, violations};
use crate::test_support::TempTree;

const INITIAL: &str = r#"from django.db import migrations, models


class Migration(migrations.Migration):
    operations = [
        migrations.CreateModel(name="Note", fields=[("id", models.BigAutoField(primary_key=True)), ("body", models.TextField())]),
    ]
"#;

const DROP_BODY: &str = r#"from django.db import migrations


class Migration(migrations.Migration):
    dependencies = [("notes", "0001_initial")]

    operations = [migrations.RemoveField(model_name="note", name="body")]
"#;

const NOTE_INITIAL: &str = "notes/migrations/0001_initial.py";
const NOTE_DROP: &str = "notes/migrations/0002_drop_body.py";

fn checked(tree: &TempTree, relative: &str, source: &str) -> usize {
    let syntax = parsed(source);
    violations(
        syntax.root_node(),
        source.as_bytes(),
        Some(&tree.path().join(relative)),
    )
    .len()
}

#[test]
fn file_loader_replays_migrations_written_on_disk() {
    let tree = TempTree::new("django-history");
    tree.write(NOTE_INITIAL, INITIAL);
    tree.write("notes/migrations/__init__.py", "");
    tree.write("notes/migrations/README.txt", "no es una migración");
    tree.write("notes/migrations/0000_helpers.py", "VALUE = 1\n");
    tree.write(NOTE_DROP, DROP_BODY);
    assert!(checked(&tree, NOTE_DROP, DROP_BODY) > 0);
    assert!(checked(&tree, NOTE_DROP, DROP_BODY) > 0);
    assert_eq!(checked(&tree, NOTE_DROP, "VALUE = 1\n"), 0);
    assert_eq!(checked(&tree, "notes/models.py", DROP_BODY), 0);
}

#[test]
fn fingerprint_changes_when_a_migration_is_added() {
    let tree = TempTree::new("django-fingerprint");
    tree.write(NOTE_INITIAL, INITIAL);
    let path = tree.path().join(NOTE_DROP);
    let before = fingerprint(&path).expect("es una migración");
    assert_eq!(fingerprint(&path), Some(before));
    tree.write(NOTE_DROP, DROP_BODY);
    assert_ne!(fingerprint(&path), Some(before));
    assert_eq!(fingerprint(Path::new("models.py")), None);
}

const FACTORY_MODULE: &str = r#"from django.conf import settings
from django.contrib.postgres.operations import AddIndexConcurrently, RemoveIndexConcurrently
from django.db.migrations import AddIndex, RemoveIndex


def add_index(model_name: str, index) -> AddIndex:
    """Agrega el índice como lo pida la configuración."""
    if settings.CONCURRENT_INDEXES:
        return AddIndexConcurrently(model_name, index)
    else:
        return AddIndex(model_name, index)


def remove_index(model_name, name):
    if not settings.CONCURRENT_INDEXES:
        return RemoveIndex(model_name, name)
    return RemoveIndexConcurrently(model_name, name)


def unknown_index(model_name, index):
    if settings.UNDECLARED:
        return AddIndex(model_name, index)
    return AddIndex(model_name, index)
"#;

const INDEXED_BY_FACTORY: &str = r#"from django.conf import settings
from django.db import migrations, models

from notes.lib.migrate import add_index as build_index, unknown_index


class Migration(migrations.Migration):
    atomic = not settings.CONCURRENT_INDEXES

    dependencies = [("notes", "0001_initial")]

    operations = [
        build_index("note", index=models.Index(fields=["body"], name="note_body_idx")),
        unknown_index(model_name="note", index=models.Index(fields=["id"], name="note_id_idx")),
    ]
"#;

const UNINDEXED_BY_FACTORY: &str = r#"from django.conf import settings
from django.db import migrations

from notes.lib.migrate import remove_index


class Migration(migrations.Migration):
    atomic = not settings.CONCURRENT_INDEXES

    dependencies = [("notes", "0002_body_index")]

    operations = [remove_index(model_name="note", name="note_body_idx")]
"#;

fn factory_sql(concurrent_indexes: &str, relative: &str, source: &str) -> String {
    let tree = TempTree::new(&format!("django-factory-{concurrent_indexes}"));
    tree.write(
        "project/settings.py",
        &format!("CONCURRENT_INDEXES = {concurrent_indexes}\n"),
    );
    tree.write("notes/lib/migrate.py", FACTORY_MODULE);
    tree.write(NOTE_INITIAL, INITIAL);
    tree.write("notes/migrations/0002_body_index.py", INDEXED_BY_FACTORY);
    tree.write("notes/migrations/0003_drop_body_index.py", UNINDEXED_BY_FACTORY);
    let path = tree.path().join(relative);
    let found = location(&path).expect("es una migración");
    rendered_sql(parsed(source).root_node(), source.as_bytes(), &found).expect("se convierte")
}

#[test]
fn project_factories_build_the_operation_their_settings_choose() {
    let concurrent = factory_sql("True", "notes/migrations/0002_body_index.py", INDEXED_BY_FACTORY);
    assert!(!concurrent.contains("BEGIN"), "{concurrent}");
    assert!(
        concurrent.contains("CREATE INDEX CONCURRENTLY \"note_body_idx\""),
        "{concurrent}"
    );
    assert!(!concurrent.contains("note_id_idx"), "{concurrent}");
    let plain = factory_sql("False", "notes/migrations/0002_body_index.py", INDEXED_BY_FACTORY);
    assert!(plain.contains("BEGIN"), "{plain}");
    assert!(plain.contains("CREATE INDEX \"note_body_idx\""), "{plain}");
    let dropped = factory_sql(
        "True",
        "notes/migrations/0003_drop_body_index.py",
        UNINDEXED_BY_FACTORY,
    );
    assert!(
        dropped.contains("DROP INDEX CONCURRENTLY IF EXISTS \"note_body_idx\""),
        "{dropped}"
    );
}
