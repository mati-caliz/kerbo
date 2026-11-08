use std::path::Path;

use super::parsed;
use crate::deploy::django::{fingerprint, violations};
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
