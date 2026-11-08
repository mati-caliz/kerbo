use std::collections::BTreeMap;
use std::sync::Arc;

use super::parsed;
use crate::deploy::django::graph::{Graph, Nodes};
use crate::deploy::django::model::ModelKey;
use crate::deploy::django::operations::MigrationKey;
use crate::deploy::django::parse::{Reader, migration};
use crate::deploy::django::settings::Settings;

const HEADER: &str = "from django.conf import settings\nfrom django.db import migrations\n\n\nclass Migration(migrations.Migration):\n";

fn nodes(declared: &[(&str, &str, &str)]) -> Nodes {
    let settings = Settings {
        user_model: ModelKey::new("account", "user"),
        ..Settings::default()
    };
    declared
        .iter()
        .map(|(app, name, body)| {
            let source = format!("{HEADER}{body}    operations = []\n");
            let tree = parsed(&source);
            let read = migration(
                tree.root_node(),
                Reader {
                    source: source.as_bytes(),
                    app,
                    settings: &settings,
                },
            )
            .expect("es una migración");
            (MigrationKey::new(app, name), Arc::new(read))
        })
        .collect::<BTreeMap<_, _>>()
}

fn plan(nodes: &Nodes, app: &str, name: &str) -> Vec<String> {
    Graph::new(nodes)
        .ancestors(&MigrationKey::new(app, name))
        .into_iter()
        .map(|key| format!("{}.{}", key.app, key.name))
        .collect()
}

#[test]
fn ancestors_follow_dependencies_in_django_order() {
    let graph = nodes(&[
        ("account", "0001_initial", ""),
        (
            "account",
            "0002_more",
            "    dependencies = [(\"account\", \"0001_initial\")]\n",
        ),
        (
            "shop",
            "0001_initial",
            "    dependencies = [migrations.swappable_dependency(settings.AUTH_USER_MODEL)]\n",
        ),
        (
            "shop",
            "0002_tag",
            "    dependencies = [(\"shop\", \"0001_initial\"), (\"tags\", \"__latest__\"), (\"auth\", \"0001_initial\")]\n",
        ),
        (
            "shop",
            "0003_unrelated",
            "    dependencies = [(\"shop\", \"0002_tag\")]\n",
        ),
        (
            "tags",
            "0001_initial",
            "    run_before = [(\"shop\", \"0001_initial\")]\n",
        ),
        (
            "tags",
            "0002_label",
            "    dependencies = [(\"tags\", \"0001_initial\")]\n",
        ),
    ]);
    assert_eq!(
        plan(&graph, "shop", "0002_tag"),
        vec![
            "tags.0001_initial",
            "tags.0002_label",
            "account.0001_initial",
            "shop.0001_initial"
        ]
    );
    assert_eq!(plan(&graph, "account", "0002_more"), vec!["account.0001_initial"]);
    assert_eq!(plan(&graph, "missing", "0001_initial"), Vec::<String>::new());
}
