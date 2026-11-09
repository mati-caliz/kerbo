use tree_sitter::Node;

use super::deferred::IndexColumn;
use super::model::Model;
use super::names::quoted;
use super::values::{boolean, text};
use crate::deploy::python_calls::Call;

const COLUMN_REFERENCE: &str = "F";
const ORDER_BY: &str = "OrderBy";
const DESCENDING_METHOD: &str = "desc";
const ASCENDING_METHOD: &str = "asc";
const DESCENDING: &str = "descending";
const NULLS_LAST: &str = "nulls_last";
const NULLS_FIRST: &str = "nulls_first";
const FUNCTIONS: &[(&str, &str)] = &[("Upper", "UPPER"), ("Lower", "LOWER")];
const OPAQUE_SQL: &str = "(true)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Column(String),
    Function {
        name: &'static str,
        arguments: Vec<Self>,
    },
    Ordered {
        inner: Box<Self>,
        descending: bool,
        nulls_last: Option<bool>,
    },
    Opaque,
}

impl Expression {
    pub fn parse(node: Node<'_>, source: &[u8]) -> Self {
        if let Some(name) = text(node, source) {
            return Self::Column(name);
        }
        Call::parse(node, source)
            .and_then(|call| Self::from_call(node, &call, source))
            .unwrap_or(Self::Opaque)
    }

    fn from_call(node: Node<'_>, call: &Call<'_>, source: &[u8]) -> Option<Self> {
        match call.method {
            COLUMN_REFERENCE => Some(Self::Column(text(call.positional(0)?, source)?)),
            ORDER_BY => Some(Self::Ordered {
                inner: Box::new(Self::parse(call.positional(0)?, source)),
                descending: call.keyword(DESCENDING).and_then(boolean).unwrap_or(false),
                nulls_last: nulls_order(call),
            }),
            DESCENDING_METHOD | ASCENDING_METHOD => {
                let object = node
                    .child_by_field_name("function")?
                    .child_by_field_name("object")?;
                Some(Self::Ordered {
                    inner: Box::new(Self::parse(object, source)),
                    descending: call.method == DESCENDING_METHOD,
                    nulls_last: nulls_order(call),
                })
            }
            method => {
                let (_, name) = FUNCTIONS.iter().find(|(class, _)| *class == method)?;
                Some(Self::Function {
                    name,
                    arguments: call
                        .all_positional()
                        .into_iter()
                        .map(|argument| Self::parse(argument, source))
                        .collect(),
                })
            }
        }
    }

    pub fn index_column(&self, model: &Model) -> IndexColumn {
        let (root, ordering) = match self {
            Self::Ordered {
                inner,
                descending,
                nulls_last,
            } => (inner.as_ref(), Some(ordering_clause(*descending, *nulls_last))),
            other @ (Self::Column(_) | Self::Function { .. } | Self::Opaque) => (other, None),
        };
        let name = match root {
            Self::Column(name) => model.column_of(name),
            other @ (Self::Function { .. } | Self::Ordered { .. } | Self::Opaque) => other
                .sql(model)
                .map_or_else(|| OPAQUE_SQL.to_owned(), |sql| format!("({sql})")),
        };
        IndexColumn {
            name,
            ordering,
            operator_class: None,
        }
    }

    fn sql(&self, model: &Model) -> Option<String> {
        match self {
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
            Self::Opaque => None,
        }
    }
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
