use std::path::Path;

use tree_sitter::Node;

use super::condition::Condition;
use super::constants::Constants;
use super::deferred::IndexColumn;
use super::enum_members::class_attribute_text;
use super::model::{Class, Model};
use super::names::quoted;
use super::values::{boolean, quoted_literal, sql_literal, text};
use crate::deploy::python_calls::Call;

const COLUMN_REFERENCE: &str = "F";
const ORDER_BY: &str = "OrderBy";
const DESCENDING_METHOD: &str = "desc";
const ASCENDING_METHOD: &str = "asc";
const DESCENDING: &str = "descending";
const NULLS_LAST: &str = "nulls_last";
const NULLS_FIRST: &str = "nulls_first";
const VALUE: &str = "Value";
const EXPRESSION_WRAPPER: &str = "ExpressionWrapper";
const RAW_SQL: &str = "RawSQL";
const CAST: &str = "Cast";
const CASE: &str = "Case";
const WHEN: &str = "When";
const THEN: &str = "then";
const DEFAULT: &str = "default";
const KEY_TRANSFORM: &str = "KeyTransform";
const KEY_TEXT_TRANSFORM: &str = "KeyTextTransform";
const GENERIC_FUNCTION: &str = "Func";
const FUNCTION_ATTRIBUTE: &str = "function";
const RANGE_BOUNDARY: &str = "RangeBoundary";
const INCLUSIVE_LOWER: &str = "inclusive_lower";
const INCLUSIVE_UPPER: &str = "inclusive_upper";
const TRUNCATION_PREFIX: &str = "Trunc";
const TRUNCATION_KINDS: &[&str] = &[
    "second", "minute", "hour", "day", "week", "month", "quarter", "year",
];
const LOOKUP_SEPARATOR: &str = "__";
const PATH_SEPARATOR: char = '.';
const JSON_OBJECT: &str = "->";
const JSON_TEXT: &str = "->>";
const SQL_NULL: &str = "NULL";
const FUNCTIONS: &[(&str, &str)] = &[
    ("Upper", "UPPER"),
    ("Lower", "LOWER"),
    ("Coalesce", "COALESCE"),
    ("Greatest", "GREATEST"),
    ("Least", "LEAST"),
    ("Abs", "ABS"),
];
const CAST_TYPES: &[(&str, &str)] = &[
    ("BigIntegerField", "bigint"),
    ("IntegerField", "integer"),
    ("SmallIntegerField", "smallint"),
    ("JSONField", "jsonb"),
    ("TextField", "text"),
    ("CharField", "varchar"),
    ("BooleanField", "boolean"),
    ("DateField", "date"),
    ("DateTimeField", "timestamp with time zone"),
    ("FloatField", "double precision"),
    ("UUIDField", "uuid"),
];
const OPAQUE_SQL: &str = "(true)";

#[derive(Debug, Clone, Copy)]
pub struct Context<'tree> {
    pub constants: Constants<'tree>,
    pub apps_root: Option<&'tree Path>,
    pub time_zone: Option<&'tree str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Column(String),
    Function {
        name: String,
        arguments: Vec<Self>,
    },
    Ordered {
        inner: Box<Self>,
        descending: bool,
        nulls_last: Option<bool>,
    },
    Literal(String),
    Raw(String),
    Key {
        object: Box<Self>,
        key: String,
        as_text: bool,
    },
    Cast {
        inner: Box<Self>,
        database_type: &'static str,
    },
    Case {
        branches: Vec<(Condition, Self)>,
        default: Option<Box<Self>>,
    },
    Truncated {
        kind: &'static str,
        inner: Box<Self>,
        time_zone: Option<String>,
    },
    Opaque,
}

impl Expression {
    pub fn parse<'tree>(node: Node<'tree>, context: Context<'tree>) -> Self {
        let source = context.constants.source;
        if let Some(name) = text(node, source) {
            return Self::Column(name);
        }
        if let Some(call) = Call::parse(node, source) {
            return Self::from_call(node, &call, context).unwrap_or(Self::Opaque);
        }
        let resolved = context.constants.resolve(node);
        if resolved != node {
            return Self::parse(resolved, context);
        }
        sql_literal(node, source).map_or_else(|| Self::Literal(SQL_NULL.to_owned()), Self::Literal)
    }

    fn from_call<'tree>(node: Node<'tree>, call: &Call<'tree>, context: Context<'tree>) -> Option<Self> {
        let source = context.constants.source;
        let first = || call.positional(0).map(|argument| Self::parse(argument, context));
        match call.method {
            COLUMN_REFERENCE => Some(Self::Column(text(call.positional(0)?, source)?)),
            ORDER_BY => Some(Self::Ordered {
                inner: Box::new(first()?),
                descending: call.keyword(DESCENDING).and_then(boolean).unwrap_or(false),
                nulls_last: nulls_order(call),
            }),
            DESCENDING_METHOD | ASCENDING_METHOD => {
                let object = node
                    .child_by_field_name("function")?
                    .child_by_field_name("object")?;
                Some(Self::Ordered {
                    inner: Box::new(Self::parse(object, context)),
                    descending: call.method == DESCENDING_METHOD,
                    nulls_last: nulls_order(call),
                })
            }
            VALUE => {
                let value = context.constants.resolve(call.positional(0)?);
                Some(Self::Literal(
                    sql_literal(value, source).unwrap_or_else(|| SQL_NULL.to_owned()),
                ))
            }
            EXPRESSION_WRAPPER => first(),
            RAW_SQL => Some(Self::Raw(text(call.positional(0)?, source)?)),
            CAST => Some(Self::Cast {
                inner: Box::new(first()?),
                database_type: cast_type(
                    call.positional(1).or_else(|| call.keyword("output_field"))?,
                    source,
                )?,
            }),
            CASE => case(call, context),
            KEY_TRANSFORM | KEY_TEXT_TRANSFORM => Some(Self::Key {
                key: key_name(call.positional(0)?, source)?,
                object: Box::new(Self::parse(call.positional(1)?, context)),
                as_text: call.method == KEY_TEXT_TRANSFORM,
            }),
            RANGE_BOUNDARY => Some(Self::Literal(range_boundary(call))),
            GENERIC_FUNCTION => Some(Self::Function {
                name: text(call.keyword(FUNCTION_ATTRIBUTE)?, source)?,
                arguments: arguments(call, context),
            }),
            method => {
                if let Some(kind) = truncation_kind(method) {
                    return Some(Self::Truncated {
                        kind,
                        inner: Box::new(first()?),
                        time_zone: context.time_zone.map(str::to_owned),
                    });
                }
                let name = FUNCTIONS
                    .iter()
                    .find(|(class, _)| *class == method)
                    .map(|(_, name)| (*name).to_owned())
                    .or_else(|| project_function(call, context.apps_root))?;
                Some(Self::Function {
                    name,
                    arguments: arguments(call, context),
                })
            }
        }
    }

    pub fn index_column(&self, model: &Model) -> IndexColumn {
        let (root, ordering) = if let Self::Ordered {
            inner,
            descending,
            nulls_last,
        } = self
        {
            (inner.as_ref(), Some(ordering_clause(*descending, *nulls_last)))
        } else {
            (self, None)
        };
        let name = root.plain_column(model).unwrap_or_else(|| {
            root.sql(model)
                .map_or_else(|| OPAQUE_SQL.to_owned(), |sql| format!("({sql})"))
        });
        IndexColumn {
            name,
            ordering,
            operator_class: None,
        }
    }

    fn plain_column(&self, model: &Model) -> Option<String> {
        let Self::Column(name) = self else {
            return None;
        };
        (!is_key_path(model, name)).then(|| model.column_of(name))
    }

    pub fn sql(&self, model: &Model) -> Option<String> {
        match self {
            Self::Column(name) if is_key_path(model, name) => key_path_sql(model, name),
            Self::Column(name) => Some(quoted(&model.column_of(name))),
            Self::Function { name, arguments } => {
                let rendered = arguments
                    .iter()
                    .map(|argument| argument.sql(model))
                    .collect::<Option<Vec<_>>>()?;
                Some(format!("{name}({})", rendered.join(", ")))
            }
            Self::Ordered {
                inner,
                descending,
                nulls_last,
            } => Some(format!(
                "{} {}",
                inner.sql(model)?,
                ordering_clause(*descending, *nulls_last)
            )),
            Self::Literal(sql) => Some(sql.clone()),
            Self::Raw(sql) => Some(format!("({sql})")),
            Self::Key { object, key, as_text } => Some(key_sql(&object.sql(model)?, key, *as_text)),
            Self::Cast { inner, database_type } => Some(format!("({})::{database_type}", inner.sql(model)?)),
            Self::Case { branches, default } => {
                let whens = branches
                    .iter()
                    .map(|(condition, result)| {
                        Some(format!(
                            " WHEN {} THEN {}",
                            condition.render(model),
                            result.sql(model)?
                        ))
                    })
                    .collect::<Option<String>>()?;
                let default = default
                    .as_ref()
                    .map_or_else(|| Some(SQL_NULL.to_owned()), |default| default.sql(model))?;
                Some(format!("CASE{whens} ELSE {default} END"))
            }
            Self::Truncated {
                kind,
                inner,
                time_zone,
            } => truncated_sql(model, kind, inner, time_zone.as_deref()),
            Self::Opaque => None,
        }
    }
}

fn arguments<'tree>(call: &Call<'tree>, context: Context<'tree>) -> Vec<Expression> {
    call.all_positional()
        .into_iter()
        .map(|argument| Expression::parse(argument, context))
        .collect()
}

fn case<'tree>(call: &Call<'tree>, context: Context<'tree>) -> Option<Expression> {
    let source = context.constants.source;
    let branches = call
        .all_positional()
        .into_iter()
        .map(|argument| {
            let when = Call::parse(argument, source).filter(|when| when.method == WHEN)?;
            let result = Expression::parse(when.keyword(THEN)?, context);
            Some((Condition::parse(argument, context.constants), result))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Expression::Case {
        branches,
        default: call
            .keyword(DEFAULT)
            .map(|value| Box::new(Expression::parse(value, context))),
    })
}

fn cast_type(node: Node<'_>, source: &[u8]) -> Option<&'static str> {
    let field = Call::parse(node, source)?;
    CAST_TYPES
        .iter()
        .find(|(class, _)| *class == field.method)
        .map(|(_, database_type)| *database_type)
}

fn key_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let name = text(node, source)?;
    Some(if name.parse::<i64>().is_ok() {
        name
    } else {
        quoted_literal(&name)
    })
}

fn key_sql(object: &str, key: &str, as_text: bool) -> String {
    let operator = if as_text { JSON_TEXT } else { JSON_OBJECT };
    format!("({object} {operator} {key})")
}

fn is_key_path(model: &Model, name: &str) -> bool {
    model.field(name).is_none() && name.contains(LOOKUP_SEPARATOR)
}

fn key_path_sql(model: &Model, name: &str) -> Option<String> {
    let (field_name, key) = name.split_once(LOOKUP_SEPARATOR)?;
    let field = model.field(field_name)?;
    let is_json = matches!(field.class, Class::Json | Class::SanitizedJson);
    if !is_json || key.contains(LOOKUP_SEPARATOR) {
        return None;
    }
    let key = if key.parse::<i64>().is_ok() {
        key.to_owned()
    } else {
        quoted_literal(key)
    };
    Some(key_sql(&quoted(&field.column()), &key, false))
}

fn truncation_kind(method: &str) -> Option<&'static str> {
    let kind = method.strip_prefix(TRUNCATION_PREFIX)?.to_lowercase();
    TRUNCATION_KINDS.iter().find(|known| **known == kind).copied()
}

fn truncated_sql(model: &Model, kind: &str, inner: &Expression, time_zone: Option<&str>) -> Option<String> {
    let Expression::Column(name) = inner else {
        return None;
    };
    let field = model.field(name)?;
    let column = quoted(&field.column());
    let value = match time_zone {
        Some(zone) if field.class == Class::DateTime => {
            format!("{column} AT TIME ZONE {}", quoted_literal(zone))
        }
        _ => column,
    };
    Some(format!("DATE_TRUNC({}, {value})", quoted_literal(kind)))
}

fn project_function(call: &Call<'_>, apps_root: Option<&Path>) -> Option<String> {
    if call.receiver.is_empty() {
        return None;
    }
    let path = format!("{}{PATH_SEPARATOR}{}", call.receiver, call.method);
    class_attribute_text(&path, FUNCTION_ATTRIBUTE, apps_root)
}

fn range_boundary(call: &Call<'_>) -> String {
    let lower = call.keyword(INCLUSIVE_LOWER).and_then(boolean).unwrap_or(true);
    let upper = call.keyword(INCLUSIVE_UPPER).and_then(boolean).unwrap_or(false);
    let bounds = format!(
        "{}{}",
        if lower { '[' } else { '(' },
        if upper { ']' } else { ')' }
    );
    quoted_literal(&bounds)
}

fn nulls_order(call: &Call<'_>) -> Option<bool> {
    if call.keyword(NULLS_LAST).and_then(boolean) == Some(true) {
        return Some(true);
    }
    (call.keyword(NULLS_FIRST).and_then(boolean) == Some(true)).then_some(false)
}

fn ordering_clause(descending: bool, nulls_last: Option<bool>) -> String {
    let direction = if descending { "DESC" } else { "ASC" };
    match nulls_last {
        Some(true) => format!("{direction} NULLS LAST"),
        Some(false) => format!("{direction} NULLS FIRST"),
        None => direction.to_owned(),
    }
}
