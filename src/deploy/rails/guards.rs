use tree_sitter::Node;

use super::arguments::{self, Arguments, name, names};
use super::ddl::singularize;
use super::schema::Schema;
use crate::syntax::named_children;

pub const CONDITIONALS: &[&str] = &["if", "unless", "elsif", "if_modifier", "unless_modifier"];
const NEGATED: &[&str] = &["unless", "unless_modifier"];
const MODIFIERS: &[&str] = &["if_modifier", "unless_modifier"];
const UNARY: &str = "unary";
const NEGATIONS: &[&str] = &["!", "not"];
const PARENTHESIZED: &str = "parenthesized_statements";
const CALL: &str = "call";
const CONNECTION: &str = "connection";
const TABLE_EXISTS: &[&str] = &["table_exists?", "data_source_exists?"];
const COLUMN_EXISTS: &str = "column_exists?";
const COLUMN_EXISTS_ARITY: usize = 2;
const INDEX_EXISTS: &str = "index_exists?";
const INDEX_NAME_EXISTS: &str = "index_name_exists?";
const FOREIGN_KEY_EXISTS: &str = "foreign_key_exists?";
const NAME: &str = "name";
const COLUMN: &str = "column";
const ID_SUFFIX: &str = "_id";

struct Guard<'context> {
    schema: &'context Schema,
    source: &'context [u8],
}

pub fn branches<'tree>(node: Node<'tree>, schema: &Schema, source: &[u8]) -> Option<Vec<Node<'tree>>> {
    let kind = node.kind();
    let negated = NEGATED.contains(&kind);
    let guard = Guard { schema, source };
    let verdict = node
        .child_by_field_name("condition")
        .and_then(|condition| guard.holds(condition))
        .map(|holds| holds != negated);
    let (taken, other) = if MODIFIERS.contains(&kind) {
        (node.child_by_field_name("body"), None)
    } else {
        (
            node.child_by_field_name("consequence"),
            node.child_by_field_name("alternative"),
        )
    };
    verdict.map(|holds| if holds { taken } else { other }.into_iter().collect())
}

impl Guard<'_> {
    fn holds(&self, condition: Node<'_>) -> Option<bool> {
        match condition.kind() {
            UNARY => {
                let operator = condition.child_by_field_name("operator")?;
                if !NEGATIONS.contains(&arguments::raw(operator, self.source).as_str()) {
                    return None;
                }
                self.holds(condition.child_by_field_name("operand")?)
                    .map(|holds| !holds)
            }
            PARENTHESIZED => {
                let children = named_children(condition);
                let [inner] = children.as_slice() else {
                    return None;
                };
                self.holds(*inner)
            }
            CALL => self.existence(condition),
            _ => None,
        }
    }

    fn existence(&self, call: Node<'_>) -> Option<bool> {
        if let Some(receiver) = call.child_by_field_name("receiver")
            && !arguments::raw(receiver, self.source).ends_with(CONNECTION)
        {
            return None;
        }
        let method = arguments::raw(call.child_by_field_name("method")?, self.source);
        let parsed = Arguments::parse(call.child_by_field_name("arguments"), self.source);
        let table = name(*parsed.positional.first()?, self.source)?;
        if TABLE_EXISTS.contains(&method.as_str()) {
            return self.schema.table_exists(&table);
        }
        if method == COLUMN_EXISTS {
            if parsed.positional.len() != COLUMN_EXISTS_ARITY || !parsed.keywords.is_empty() {
                return None;
            }
            return self
                .schema
                .column_exists(&table, &parsed.name_at(1, self.source)?);
        }
        if method == INDEX_NAME_EXISTS {
            let index = parsed.name_at(1, self.source)?;
            return self.schema.index_exists(&table, &[], Some(&index));
        }
        let named = parsed.keyword_name(NAME, self.source);
        if method == INDEX_EXISTS {
            let columns = parsed
                .positional
                .get(1)
                .map(|columns| names(*columns, self.source))
                .unwrap_or_default();
            if columns.is_empty() && parsed.positional.len() > 1 {
                return None;
            }
            return self.schema.index_exists(&table, &columns, named.as_deref());
        }
        if method != FOREIGN_KEY_EXISTS {
            return None;
        }
        let column = parsed.keyword_name(COLUMN, self.source).or_else(|| {
            let to_table = parsed.name_at(1, self.source)?;
            Some(format!("{}{ID_SUFFIX}", singularize(&to_table)))
        })?;
        self.schema.foreign_key_exists(&table, &column, named.as_deref())
    }
}
