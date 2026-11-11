use tree_sitter::Node;

use super::arguments::{Arguments, names, raw, string_content};
use super::sha256::hex_digest;

const MAXIMUM_INDEX_NAME: usize = 62;
const INDEX_PREFIX: &str = "index_";
const SHORT_INDEX_PREFIX: &str = "idx_on_";
const FOREIGN_KEY_PREFIX: &str = "fk_rails_";
const CHECK_PREFIX: &str = "chk_rails_";
const HASH_DIGITS: usize = 10;
const ID_SUFFIX: &str = "_id";
const TYPE_SUFFIX: &str = "_type";
const PRIMARY_KEY: &str = "id";
const CONCURRENTLY: &str = "concurrently";
const ALGORITHM: &str = "algorithm";
const UNIQUE: &str = "unique";
const USING: &str = "using";
const WHERE: &str = "where";
const NAME: &str = "name";
const IF_NOT_EXISTS: &str = "if_not_exists";
const COLUMN: &str = "column";
const TO_TABLE: &str = "to_table";
const PRIMARY_KEY_OPTION: &str = "primary_key";
const ON_DELETE: &str = "on_delete";
const VALIDATE: &str = "validate";
const ON_DELETE_ACTIONS: &[(&str, &str)] = &[
    ("cascade", "CASCADE"),
    ("nullify", "SET NULL"),
    ("restrict", "RESTRICT"),
];
const EXPRESSION_MARKERS: &[char] = &['(', ' '];

pub fn quote(identifier: &str) -> String {
    format!("\"{identifier}\"")
}

fn short_hash(text: &str) -> String {
    hex_digest(text.as_bytes()).chars().take(HASH_DIGITS).collect()
}

pub fn index_name(table: &str, columns: &[String]) -> String {
    let name = format!("{INDEX_PREFIX}{table}_on_{}", columns.join("_and_"));
    if name.len() <= MAXIMUM_INDEX_NAME {
        return name;
    }
    let suffix = format!("_{}", short_hash(&name));
    let short_name = format!("{SHORT_INDEX_PREFIX}{}", columns.join("_"));
    let limit = MAXIMUM_INDEX_NAME - suffix.len();
    let truncated: String = short_name.chars().take(limit).collect();
    format!("{truncated}{suffix}")
}

pub fn foreign_key_name(table: &str, column: &str) -> String {
    format!(
        "{FOREIGN_KEY_PREFIX}{}",
        short_hash(&format!("{table}_{column}_fk"))
    )
}

pub fn check_name(table: &str, expression: &str) -> String {
    format!(
        "{CHECK_PREFIX}{}",
        short_hash(&format!("{table}_{expression}_chk"))
    )
}

pub fn pluralize(word: &str) -> String {
    if let Some(stem) = word
        .strip_suffix('y')
        .filter(|stem| !stem.ends_with(['a', 'e', 'o', 'u']))
    {
        return format!("{stem}ies");
    }
    if word.ends_with(['s', 'x']) || word.ends_with("ch") || word.ends_with("sh") {
        return format!("{word}es");
    }
    format!("{word}s")
}

pub fn singularize(word: &str) -> String {
    if let Some(stem) = ["statuses", "aliases"]
        .iter()
        .find(|plural| word.ends_with(*plural))
        .and_then(|_| word.strip_suffix("es"))
    {
        return stem.to_owned();
    }
    if let Some(stem) = word.strip_suffix("ies") {
        return format!("{stem}y");
    }
    if let Some(stem) = ["ches", "shes", "sses", "xes"]
        .iter()
        .find_map(|suffix| word.strip_suffix(suffix).map(|stem| (stem, suffix)))
    {
        return format!("{}{}", stem.0, &stem.1[..stem.1.len() - 2]);
    }
    word.strip_suffix('s').unwrap_or(word).to_owned()
}

pub struct IndexColumns {
    pub rendered: String,
    pub names: Vec<String>,
}

impl IndexColumns {
    pub fn plain(columns: &[String]) -> Self {
        Self {
            rendered: render_columns(columns),
            names: columns.to_vec(),
        }
    }

    fn parse(node: Node<'_>, source: &[u8]) -> Self {
        let expression = string_content(node, source).filter(|text| text.contains(EXPRESSION_MARKERS));
        if let Some(expression) = expression {
            return Self {
                rendered: expression.clone(),
                names: vec![expression],
            };
        }
        Self::plain(&names(node, source))
    }
}

pub fn create_index(table: &str, columns: Node<'_>, arguments: &Arguments<'_>, source: &[u8]) -> String {
    create_index_on(table, &IndexColumns::parse(columns, source), arguments, source)
}

fn render_columns(columns: &[String]) -> String {
    columns
        .iter()
        .map(|column| quote(column))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn create_index_on(
    table: &str,
    columns: &IndexColumns,
    arguments: &Arguments<'_>,
    source: &[u8],
) -> String {
    let name = arguments
        .keyword_name(NAME, source)
        .unwrap_or_else(|| index_name(table, &columns.names));
    let unique = if arguments.is_true(UNIQUE) { "UNIQUE " } else { "" };
    let concurrently = if is_concurrent(arguments, source) {
        "CONCURRENTLY "
    } else {
        ""
    };
    let if_not_exists = if arguments.is_true(IF_NOT_EXISTS) {
        "IF NOT EXISTS "
    } else {
        ""
    };
    let using = arguments
        .keyword_name(USING, source)
        .map(|method| format!("USING {method} "))
        .unwrap_or_default();
    let condition = arguments
        .keyword(WHERE)
        .map(|node| {
            format!(
                " WHERE {}",
                string_content(node, source).unwrap_or_else(|| raw(node, source))
            )
        })
        .unwrap_or_default();
    format!(
        "CREATE {unique}INDEX {concurrently}{if_not_exists}{} ON {} {using}({}){condition}",
        quote(&name),
        quote(table),
        columns.rendered
    )
}

pub fn drop_index(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> String {
    let name = arguments.keyword_name(NAME, source).unwrap_or_else(|| {
        let columns = arguments
            .keyword(COLUMN)
            .or_else(|| arguments.positional.first().copied())
            .map(|node| names(node, source))
            .unwrap_or_default();
        index_name(table, &columns)
    });
    let concurrently = if is_concurrent(arguments, source) {
        "CONCURRENTLY "
    } else {
        ""
    };
    format!("DROP INDEX {concurrently}{}", quote(&name))
}

fn is_concurrent(arguments: &Arguments<'_>, source: &[u8]) -> bool {
    arguments.keyword_name(ALGORITHM, source).as_deref() == Some(CONCURRENTLY)
}

pub struct ForeignKey {
    pub name: Option<String>,
    pub column: String,
    pub to_table: String,
    pub primary_key: String,
    pub on_delete: Option<&'static str>,
    pub validate: bool,
}

impl ForeignKey {
    pub fn new(to_table: String, arguments: &Arguments<'_>, source: &[u8]) -> Self {
        let column = arguments
            .keyword_name(COLUMN, source)
            .unwrap_or_else(|| format!("{}{ID_SUFFIX}", singularize(&to_table)));
        Self::with_column(column, to_table, arguments, source)
    }

    pub fn with_column(column: String, to_table: String, arguments: &Arguments<'_>, source: &[u8]) -> Self {
        let on_delete = arguments.keyword_name(ON_DELETE, source).and_then(|action| {
            ON_DELETE_ACTIONS
                .iter()
                .find_map(|(rails, sql)| (*rails == action).then_some(*sql))
        });
        Self {
            name: arguments.keyword_name(NAME, source),
            column,
            to_table: arguments.keyword_name(TO_TABLE, source).unwrap_or(to_table),
            primary_key: arguments
                .keyword_name(PRIMARY_KEY_OPTION, source)
                .unwrap_or_else(|| PRIMARY_KEY.to_owned()),
            on_delete,
            validate: !arguments.is_false(VALIDATE),
        }
    }

    pub fn constraint(&self, table: &str) -> String {
        let on_delete = self
            .on_delete
            .map(|action| format!(" ON DELETE {action}"))
            .unwrap_or_default();
        format!(
            "CONSTRAINT {}\nFOREIGN KEY ({})\n  REFERENCES {} ({}){on_delete}",
            quote(&self.constraint_name(table)),
            quote(&self.column),
            quote(&self.to_table),
            quote(&self.primary_key)
        )
    }

    pub fn constraint_name(&self, table: &str) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| foreign_key_name(table, &self.column))
    }

    pub fn add(&self, table: &str) -> String {
        let not_valid = if self.validate { "" } else { " NOT VALID" };
        format!(
            "ALTER TABLE {} ADD {}{not_valid}",
            quote(table),
            self.constraint(table)
        )
    }
}

pub fn reference_columns(reference: &str) -> (String, String) {
    (
        format!("{reference}{ID_SUFFIX}"),
        format!("{reference}{TYPE_SUFFIX}"),
    )
}
