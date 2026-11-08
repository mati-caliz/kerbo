use tree_sitter::Node;

use super::model::{Class, Field, Model};
use super::names::quoted;
use super::values::{boolean, elements, is_sequence, quoted_literal, raw, text};
use crate::deploy::python_calls::Call;

const Q_CLASS: &str = "Q";
const DECIMAL_CONSTRUCTOR: &str = "Decimal";
const LOOKUP_SEPARATOR: &str = "__";
const PRIMARY_KEY_ALIAS: &str = "pk";
const UNARY: &str = "unary_operator";
const BINARY: &str = "binary_operator";
const PARENTHESIZED: &str = "parenthesized_expression";
const KEYWORD_ARGUMENT: &str = "keyword_argument";
const INVERT: &str = "~";
const OR_OPERATOR: &str = "|";
const AND_OPERATOR: &str = "&";
const NUMERIC_LITERALS: &[&str] = &["integer", "float"];
const STRING_LITERALS: &[&str] = &["string", "concatenated_string"];
const OPAQUE_SQL: &str = "true";
const EXACT: &str = "exact";
const IS_NULL: &str = "isnull";
const IN: &str = "in";
const COMPARISONS: &[(&str, &str)] = &[("gt", ">"), ("gte", ">="), ("lt", "<"), ("lte", "<=")];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Connector {
    And,
    Or,
}

impl Connector {
    const fn separator(self) -> &'static str {
        match self {
            Self::And => " AND ",
            Self::Or => " OR ",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    Literal(String),
    Boolean(bool),
    Null,
    List(Vec<Self>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Child {
    Lookup { path: String, value: Value },
    Node(Tree),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Tree {
    connector: Connector,
    negated: bool,
    children: Vec<Child>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition(Option<Tree>);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Where {
    Leaf(String),
    Node {
        connector: Connector,
        negated: bool,
        children: Vec<Self>,
    },
}

impl Condition {
    pub fn parse<'tree>(node: Node<'tree>, source: &'tree [u8]) -> Self {
        Self(expression(node, source))
    }

    pub const fn opaque() -> Self {
        Self(None)
    }

    pub fn render(&self, model: &Model) -> String {
        self.0
            .as_ref()
            .and_then(|tree| where_clause(tree, false, model))
            .and_then(|clause| clause.render())
            .unwrap_or_else(|| OPAQUE_SQL.to_owned())
    }
}

fn expression<'tree>(node: Node<'tree>, source: &'tree [u8]) -> Option<Tree> {
    match node.kind() {
        PARENTHESIZED => expression(node.named_child(0)?, source),
        UNARY if operator(node, source)? == INVERT => {
            let mut inverted = expression(node.child_by_field_name("argument")?, source)?;
            inverted.negated = !inverted.negated;
            Some(inverted)
        }
        BINARY => {
            let connector = match operator(node, source)? {
                OR_OPERATOR => Connector::Or,
                AND_OPERATOR => Connector::And,
                _ => return None,
            };
            let left = expression(node.child_by_field_name("left")?, source)?;
            let right = expression(node.child_by_field_name("right")?, source)?;
            Some(combine(left, right, connector))
        }
        _ => q_call(node, source),
    }
}

fn operator<'source>(node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    node.child_by_field_name("operator")?.utf8_text(source).ok()
}

fn q_call<'tree>(node: Node<'tree>, source: &'tree [u8]) -> Option<Tree> {
    let call = Call::parse(node, source).filter(|call| call.method == Q_CLASS)?;
    let mut children = Vec::new();
    for argument in call.all_positional() {
        children.push(positional_child(argument, source)?);
    }
    let mut keywords = Vec::new();
    for argument in call
        .arguments
        .iter()
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
    {
        let name = argument.child_by_field_name("name")?.utf8_text(source).ok()?;
        keywords.push((
            name.to_owned(),
            value(argument.child_by_field_name("value")?, source)?,
        ));
    }
    keywords.sort_by(|left, right| left.0.cmp(&right.0));
    children.extend(
        keywords
            .into_iter()
            .map(|(path, value)| Child::Lookup { path, value }),
    );
    Some(Tree {
        connector: Connector::And,
        negated: false,
        children,
    })
}

fn positional_child<'tree>(node: Node<'tree>, source: &'tree [u8]) -> Option<Child> {
    if is_sequence(node) {
        let parts = elements(node);
        let [path, rhs] = parts.as_slice() else {
            return None;
        };
        return Some(Child::Lookup {
            path: text(*path, source)?,
            value: value(*rhs, source)?,
        });
    }
    expression(node, source).map(Child::Node)
}

fn value<'tree>(node: Node<'tree>, source: &'tree [u8]) -> Option<Value> {
    if node.kind() == "none" {
        return Some(Value::Null);
    }
    if let Some(flag) = boolean(node) {
        return Some(Value::Boolean(flag));
    }
    if STRING_LITERALS.contains(&node.kind()) {
        return Some(Value::Literal(quoted_literal(&text(node, source)?)));
    }
    if is_sequence(node) {
        return elements(node)
            .into_iter()
            .map(|element| value(element, source))
            .collect::<Option<Vec<_>>>()
            .map(Value::List);
    }
    numeric(node, source).map(Value::Literal)
}

fn numeric<'tree>(node: Node<'tree>, source: &'tree [u8]) -> Option<String> {
    if let Some(call) = Call::parse(node, source).filter(|call| call.method == DECIMAL_CONSTRUCTOR) {
        let argument = call.positional(0)?;
        return text(argument, source).or_else(|| numeric(argument, source));
    }
    let literal = raw(node, source);
    let is_number = NUMERIC_LITERALS.contains(&node.kind())
        || (node.kind() == UNARY && literal.trim().parse::<f64>().is_ok());
    is_number.then_some(literal)
}

fn combine(left: Tree, right: Tree, connector: Connector) -> Tree {
    if left.children.is_empty() {
        return right;
    }
    if right.children.is_empty() {
        return left;
    }
    let mut combined = Tree {
        connector,
        negated: false,
        children: Vec::new(),
    };
    for side in [left, right] {
        if !side.negated && (side.connector == connector || side.children.len() == 1) {
            combined.children.extend(side.children);
        } else {
            combined.children.push(Child::Node(side));
        }
    }
    combined
}

fn where_clause(tree: &Tree, negated: bool, model: &Model) -> Option<Where> {
    let current_negated = negated ^ tree.negated;
    let mut children = Vec::new();
    for child in &tree.children {
        let clause = match child {
            Child::Node(inner) => where_clause(inner, current_negated, model)?,
            Child::Lookup { path, value } => lookup_clause(path, value, current_negated, model)?,
        };
        match clause {
            Where::Node {
                connector,
                negated: false,
                children: inner,
            } if connector == tree.connector || inner.len() == 1 => children.extend(inner),
            other @ (Where::Leaf(_) | Where::Node { .. }) => children.push(other),
        }
    }
    Some(Where::Node {
        connector: tree.connector,
        negated: tree.negated,
        children,
    })
}

fn lookup_clause(path: &str, value: &Value, negated: bool, model: &Model) -> Option<Where> {
    let parts: Vec<&str> = path.split(LOOKUP_SEPARATOR).collect();
    let (name, requested) = match *parts.as_slice() {
        [name] => (name, EXACT),
        [name, lookup] => (name, lookup),
        _ => return None,
    };
    let field = target_field(model, name)?;
    let (lookup, value) = match value {
        Value::Null if requested == EXACT => (IS_NULL, &Value::Boolean(true)),
        Value::Null => return None,
        other @ (Value::Literal(_) | Value::Boolean(_) | Value::List(_)) => (requested, other),
    };
    let column = quoted(&field.column());
    let mut leaves = vec![Where::Leaf(lookup_sql(field, &column, lookup, value)?)];
    if negated && lookup != IS_NULL && field.null() {
        leaves.push(Where::Leaf(format!("{column} IS NOT NULL")));
    }
    Some(Where::Node {
        connector: Connector::And,
        negated: false,
        children: leaves,
    })
}

fn target_field<'model>(model: &'model Model, name: &str) -> Option<&'model Field> {
    let field = if name == PRIMARY_KEY_ALIAS {
        model.primary_key()
    } else {
        model.field(name).or_else(|| {
            model
                .fields
                .iter()
                .find(|field| field.is_foreign_key() && field.column() == name)
        })
    }?;
    (!field.is_many_to_many()).then_some(field)
}

fn lookup_sql(field: &Field, column: &str, lookup: &str, value: &Value) -> Option<String> {
    match (lookup, value) {
        (EXACT, Value::Boolean(flag)) if field.class == Class::Boolean => Some(if *flag {
            column.to_owned()
        } else {
            format!("NOT {column}")
        }),
        (EXACT, rhs) => Some(format!("{column} = {}", scalar(rhs)?)),
        (IS_NULL, Value::Boolean(true)) => Some(format!("{column} IS NULL")),
        (IS_NULL, Value::Boolean(false)) => Some(format!("{column} IS NOT NULL")),
        (IN, Value::List(items)) => {
            let mut rendered: Vec<String> = Vec::new();
            for item in items.iter().filter(|item| **item != Value::Null) {
                let sql = scalar(item)?;
                if !rendered.contains(&sql) {
                    rendered.push(sql);
                }
            }
            (!rendered.is_empty()).then(|| format!("{column} IN ({})", rendered.join(", ")))
        }
        (comparison, rhs) => {
            let (_, symbol) = COMPARISONS.iter().find(|(name, _)| *name == comparison)?;
            Some(format!("{column} {symbol} {}", scalar(rhs)?))
        }
    }
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::Literal(sql) => Some(sql.clone()),
        Value::Boolean(flag) => Some(flag.to_string()),
        Value::Null | Value::List(_) => None,
    }
}

impl Where {
    fn render(&self) -> Option<String> {
        match self {
            Self::Leaf(sql) => Some(sql.clone()),
            Self::Node {
                connector,
                negated,
                children,
            } => {
                let parts = children.iter().map(Self::render).collect::<Option<Vec<_>>>()?;
                if parts.is_empty() {
                    return None;
                }
                let joined = parts.join(connector.separator());
                Some(if *negated {
                    format!("NOT ({joined})")
                } else if parts.len() > 1 {
                    format!("({joined})")
                } else {
                    joined
                })
            }
        }
    }
}
