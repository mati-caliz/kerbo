use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::nodes::COMMENT;
use super::strings::static_string;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const LINE_PREFIX: &str = "//";
const HASH_BANG: &str = "hash_bang_line";
const IMPORT: &str = "import_statement";
const REQUIRE_CLAUSE: &str = "import_require_clause";
const SOURCE_FIELD: &str = "source";
const KIND_GROUP: &str = "kind";
const MODULE_GROUP: &str = "module";
const PATH: &str = "path";
const TYPES: &str = "types";

static REFERENCE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"^/\s*<reference\s*(?<kind>types|path|lib)\s*=\s*["|'](?<module>.*)["|']"#).ok()
});

struct Reference<'tree> {
    comment: Node<'tree>,
    kind: String,
    module: String,
}

// `triple-slash-reference` con las opciones de fábrica: `path` nunca y `types` sólo si el mismo
// módulo no se importa (cada import que lo repite es un aviso más sobre el comentario).
pub fn triple_slash_references(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let references = leading_references(context);
    let paths = references
        .iter()
        .filter(|reference| reference.kind == PATH)
        .map(|reference| {
            report(
                EslintRule::TripleSlashReference,
                reference.comment,
                &reference.module,
            )
        });
    let imported = context
        .of_kind(&[IMPORT])
        .filter_map(|import| imported_module(import, context))
        .flat_map(|module| {
            references
                .iter()
                .filter(move |reference| reference.kind == TYPES && reference.module == module)
        })
        .map(|reference| {
            report(
                EslintRule::TripleSlashReference,
                reference.comment,
                &reference.module,
            )
        });
    paths.chain(imported).collect()
}

fn leading_references<'tree>(context: &Context<'tree, '_>) -> Vec<Reference<'tree>> {
    let mut cursor = context.root.walk();
    context
        .root
        .named_children(&mut cursor)
        .take_while(|child| [COMMENT, HASH_BANG].contains(&child.kind()))
        .filter(|child| child.kind() == COMMENT)
        .filter_map(|comment| {
            let value = context.text(comment).strip_prefix(LINE_PREFIX)?;
            let captures = REFERENCE.as_ref()?.captures(value)?;
            Some(Reference {
                comment,
                kind: captures.name(KIND_GROUP)?.as_str().to_owned(),
                module: captures.name(MODULE_GROUP)?.as_str().to_owned(),
            })
        })
        .collect()
}

fn imported_module(import: Node<'_>, context: &Context<'_, '_>) -> Option<String> {
    let source = import.child_by_field_name(SOURCE_FIELD).or_else(|| {
        let mut cursor = import.walk();
        import
            .named_children(&mut cursor)
            .find(|child| child.kind() == REQUIRE_CLAUSE)
            .and_then(|clause| clause.child_by_field_name(SOURCE_FIELD))
    })?;
    static_string(source, context.source)
}
