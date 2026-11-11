use tree_sitter::Node;

use super::model::{Class, FieldDefault};
use super::values::{quoted_literal, raw, sql_literal, text};
use crate::deploy::python_calls::Call;

pub const DB_VALUE_WRAPPER: &str = "Value";
const NOW_FUNCTION: &str = "Now";
pub const NOW_SQL: &str = "(STATEMENT_TIMESTAMP())";
const SQL_NULL: &str = "NULL";
const NONE: &str = "none";
const DECIMAL_CONSTRUCTOR: &str = "Decimal";
const DURATION_CONSTRUCTOR: &str = "timedelta";
const ENVIRONMENT_RECEIVERS: &[&str] = &["os.environ", "environ"];
const ENVIRONMENT_READER: &str = "get";
const ENVIRONMENT_FALLBACK: usize = 1;
const EMPTY_POSTGRES_ARRAY: &str = "'{}'";
const INTERVAL_CAST: &str = "::interval";
const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_DAY: i64 = 86_400;
const DURATION_UNITS: &[(&str, i64)] = &[
    ("weeks", 604_800),
    ("days", SECONDS_PER_DAY),
    ("hours", SECONDS_PER_HOUR),
    ("minutes", SECONDS_PER_MINUTE),
    ("seconds", 1),
];
const POSITIONAL_DURATION_UNITS: &[i64] = &[SECONDS_PER_DAY, 1];
const JSON_CAST: &str = "::jsonb";
const EMPTY_OBJECT: &str = "{}";
const EMPTY_ARRAY: &str = "[]";
const OBJECT_DEFAULTS: &[&str] = &["dict", "dictionary"];
const ARRAY_DEFAULTS: &[&str] = &["list", "tuple"];
const ARRAY_LITERALS: &[&str] = &["list", "tuple"];
const OBJECT_LITERAL: &str = "dictionary";
pub const SANITIZED_NULL_DEFAULT: &str = "'null'";
pub const NOW_DEFAULT: &str = "'2000-01-01 00:00:00+00:00'::timestamptz";
const TODAY_DEFAULT: &str = "'2000-01-01'::date";
const CLOCK_DEFAULT: &str = "'00:00:00'::time";
const UUID_DEFAULT: &str = "'00000000000000000000000000000000'::uuid";
pub fn default(node: Node<'_>, source: &[u8], class: Class) -> FieldDefault {
    let value = environment_fallback(node, source).unwrap_or(node);
    if value.kind() == NONE {
        return FieldDefault::Null;
    }
    typed_default(value, source, class)
        .or_else(|| sql_literal(value, source))
        .map_or(FieldDefault::Null, FieldDefault::Literal)
}

fn environment_fallback<'tree>(node: Node<'tree>, source: &'tree [u8]) -> Option<Node<'tree>> {
    let call = Call::parse(node, source)?;
    let reads_environment =
        call.method == ENVIRONMENT_READER && ENVIRONMENT_RECEIVERS.contains(&call.receiver);
    reads_environment
        .then(|| call.positional(ENVIRONMENT_FALLBACK))
        .flatten()
}

fn typed_default(node: Node<'_>, source: &[u8], class: Class) -> Option<String> {
    if let Some(placeholder) = placeholder_default(class) {
        return Some(placeholder.to_owned());
    }
    if is_integer(class) {
        return decimal_argument(node, source);
    }
    if class == Class::Json {
        return json_default(node, source);
    }
    if class == Class::SanitizedJson {
        return empty_json(node, source).map(quoted_literal);
    }
    if class == Class::Array {
        return empty_json(node, source)
            .filter(|json| *json == EMPTY_ARRAY)
            .map(|_| EMPTY_POSTGRES_ARRAY.to_owned());
    }
    if class == Class::Decimal {
        return decimal_argument(node, source).or_else(|| numeric_text(node, source));
    }
    (class == Class::Duration)
        .then(|| duration(node, source))
        .flatten()
}

fn json_default(node: Node<'_>, source: &[u8]) -> Option<String> {
    empty_json(node, source).map(|json| format!("{}{JSON_CAST}", quoted_literal(json)))
}

const fn is_integer(class: Class) -> bool {
    matches!(
        class,
        Class::Integer
            | Class::BigInteger
            | Class::SmallInteger
            | Class::PositiveInteger
            | Class::PositiveBigInteger
            | Class::PositiveSmallInteger
    )
}

fn decimal_argument(node: Node<'_>, source: &[u8]) -> Option<String> {
    let argument = Call::parse(node, source)
        .filter(|call| call.method == DECIMAL_CONSTRUCTOR)?
        .positional(0)?;
    text(argument, source).or_else(|| numeric_text(argument, source))
}

fn numeric_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    let value = text(node, source).unwrap_or_else(|| raw(node, source));
    value
        .trim()
        .parse::<f64>()
        .is_ok()
        .then(|| value.trim().to_owned())
}

fn duration(node: Node<'_>, source: &[u8]) -> Option<String> {
    let call = Call::parse(node, source).filter(|call| call.method == DURATION_CONSTRUCTOR)?;
    let integer = |argument: Node<'_>| raw(argument, source).parse::<i64>().ok();
    let mut total_seconds: i64 = 0;
    for (index, multiplier) in POSITIONAL_DURATION_UNITS.iter().enumerate() {
        if let Some(argument) = call.positional(index) {
            total_seconds = total_seconds.checked_add(integer(argument)?.checked_mul(*multiplier)?)?;
        }
    }
    for (keyword, multiplier) in DURATION_UNITS {
        if let Some(argument) = call.keyword(keyword) {
            total_seconds = total_seconds.checked_add(integer(argument)?.checked_mul(*multiplier)?)?;
        }
    }
    Some(format!(
        "{}{INTERVAL_CAST}",
        quoted_literal(&python_duration(total_seconds))
    ))
}

fn python_duration(total_seconds: i64) -> String {
    let days = total_seconds.div_euclid(SECONDS_PER_DAY);
    let seconds = total_seconds.rem_euclid(SECONDS_PER_DAY);
    let clock = format!(
        "{}:{:02}:{:02}",
        seconds / SECONDS_PER_HOUR,
        seconds % SECONDS_PER_HOUR / SECONDS_PER_MINUTE,
        seconds % SECONDS_PER_MINUTE
    );
    match days {
        0 => clock,
        1 | -1 => format!("{days} day {clock}"),
        _ => format!("{days} days {clock}"),
    }
}

fn placeholder_default(class: Class) -> Option<&'static str> {
    [
        (Class::DateTime, NOW_DEFAULT),
        (Class::Date, TODAY_DEFAULT),
        (Class::Uuid, UUID_DEFAULT),
    ]
    .into_iter()
    .find_map(|(placeholder_class, placeholder)| (placeholder_class == class).then_some(placeholder))
}

pub const fn automatic_now_default(class: Class) -> &'static str {
    if matches!(class, Class::Date) {
        TODAY_DEFAULT
    } else if matches!(class, Class::Time) {
        CLOCK_DEFAULT
    } else {
        NOW_DEFAULT
    }
}

fn empty_json(node: Node<'_>, source: &[u8]) -> Option<&'static str> {
    let reference = raw(node, source);
    let is_empty_literal = node.named_child_count() == 0;
    if OBJECT_DEFAULTS.contains(&reference.as_str()) || (node.kind() == OBJECT_LITERAL && is_empty_literal) {
        return Some(EMPTY_OBJECT);
    }
    if ARRAY_DEFAULTS.contains(&reference.as_str())
        || (ARRAY_LITERALS.contains(&node.kind()) && is_empty_literal)
    {
        return Some(EMPTY_ARRAY);
    }
    None
}

pub fn database_default(node: Node<'_>, source: &[u8], class: Class) -> Option<String> {
    let call = Call::parse(node, source);
    if call
        .as_ref()
        .is_some_and(|call| call.method == NOW_FUNCTION && call.arguments.is_empty())
    {
        return Some(NOW_SQL.to_owned());
    }
    let wrapped = call
        .filter(|call| call.method == DB_VALUE_WRAPPER)
        .and_then(|call| call.positional(0));
    let value = wrapped.unwrap_or(node);
    if value.kind() == NONE {
        return Some(SQL_NULL.to_owned());
    }
    let typed = if class == Class::Json {
        json_default(value, source)
    } else if class == Class::Decimal {
        decimal_argument(value, source)
    } else {
        None
    };
    typed.or_else(|| sql_literal(value, source))
}
