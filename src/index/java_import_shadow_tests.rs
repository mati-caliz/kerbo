use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;
use crate::language::Language;
use crate::lint::java_import_shadows::import_shadows;
use std::path::Path;

const BAG: &str = "package app.model;

public interface Bag {
    interface Item {
    }
}
";

const STORE: &str = "package app.model;

public class Store implements Bag {
}
";

const USES: &str = "package app;

import app.model.Bag.Item;
import app.model.Store;
import sample.cli.Child;
import sample.cli.Holder.Key;

class Uses {
    static class Shelf extends Store {
        Item first() {
            return null;
        }
    }

    static class Plain {
        Item second() {
            return null;
        }
    }

    static class Keys extends Child {
        Key key() {
            return null;
        }
    }
}
";

fn with_shadows(source: &str) -> crate::index::java::JavaSummary {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&Language::Java.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    let mut summary = summary_of(source);
    summary.import_shadows = import_shadows(tree.root_node(), source.as_bytes());
    summary
}

#[test]
fn an_import_shadowed_by_an_inherited_member_type_in_every_use_is_unnecessary() {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/annotated.jar");
    let summaries = [summary_of(BAG), summary_of(STORE), with_shadows(USES)];
    let files: Vec<IndexedSummary<'_>> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
    let found: Vec<(usize, String)> = project_lints(&files, &Library::load(&[jar]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "unused-import")
        .map(|finding| (finding.line, finding.subject))
        .collect();
    assert_eq!(found, [(6, "sample.cli.Holder.Key".to_owned())]);
}
