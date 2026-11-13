use tree_sitter::Node;

use super::arguments::Arguments;
use super::columns::Column;
use super::ddl::{ForeignKey, IndexColumns, create_index_on, pluralize, reference_columns};

const TYPE: &str = "type";
const NULL: &str = "null";
const INDEX: &str = "index";
const POLYMORPHIC: &str = "polymorphic";
const FOREIGN_KEY: &str = "foreign_key";
const HASH: &str = "hash";
const TRUE: &str = "true";
const DEFAULT_TYPE: &str = "bigint";
const INTEGER_TYPE: &str = "integer";
const POLYMORPHIC_TYPE: &str = "character varying";
const REFERENCE_TYPES: &[(&str, &str)] = &[
    ("uuid", "uuid"),
    ("integer", "integer"),
    ("string", POLYMORPHIC_TYPE),
    ("bigint", DEFAULT_TYPE),
];

pub struct Reference {
    pub columns: Vec<Column>,
    pub index: Option<String>,
    pub foreign_key: Option<ForeignKey>,
}

pub fn reference(table: &str, name: &str, arguments: &Arguments<'_>, source: &[u8]) -> Reference {
    let (id, kind) = reference_columns(name);
    let polymorphic = arguments.is_true(POLYMORPHIC);
    let not_null = arguments.is_false(NULL);
    let id_type = arguments
        .keyword_name(TYPE, source)
        .and_then(|declared| {
            REFERENCE_TYPES
                .iter()
                .find_map(|(rails, sql)| (*rails == declared).then_some(*sql))
        })
        .unwrap_or(if arguments.compatibility.integer_ids {
            INTEGER_TYPE
        } else {
            DEFAULT_TYPE
        });
    let mut columns = vec![Column {
        name: id.clone(),
        sql_type: id_type.to_owned(),
        default: None,
        not_null,
        primary_key: false,
    }];
    let mut indexed = vec![id.clone()];
    if polymorphic {
        columns.push(Column {
            name: kind.clone(),
            sql_type: POLYMORPHIC_TYPE.to_owned(),
            default: None,
            not_null,
            primary_key: false,
        });
        indexed.insert(0, kind);
    }
    Reference {
        index: reference_index(table, &indexed, arguments, source),
        foreign_key: (!polymorphic)
            .then(|| foreign_key(name, id, arguments.keyword(FOREIGN_KEY), source))
            .flatten(),
        columns,
    }
}

fn reference_index(
    table: &str,
    columns: &[String],
    arguments: &Arguments<'_>,
    source: &[u8],
) -> Option<String> {
    if arguments.compatibility.indexed_references {
        index(table, columns, arguments.keyword(INDEX), source)
    } else {
        column_index(table, columns, arguments, source)
    }
}

pub fn column_index(
    table: &str,
    columns: &[String],
    arguments: &Arguments<'_>,
    source: &[u8],
) -> Option<String> {
    arguments
        .keyword(INDEX)
        .and_then(|option| index(table, columns, Some(option), source))
}

fn index(table: &str, columns: &[String], option: Option<Node<'_>>, source: &[u8]) -> Option<String> {
    let options = match option {
        None => Arguments::default(),
        Some(node) if node.kind() == HASH => Arguments::parse(Some(node), source),
        Some(node) if node.kind() == TRUE => Arguments::default(),
        Some(_) => return None,
    };
    Some(create_index_on(
        table,
        &IndexColumns::plain(columns),
        &options,
        source,
    ))
}

fn foreign_key(name: &str, column: String, option: Option<Node<'_>>, source: &[u8]) -> Option<ForeignKey> {
    let option = option?;
    let options = match option.kind() {
        HASH => Arguments::parse(Some(option), source),
        TRUE => Arguments::default(),
        _ => return None,
    };
    Some(ForeignKey::with_column(column, pluralize(name), &options, source))
}
