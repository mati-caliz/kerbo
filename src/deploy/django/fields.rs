use tree_sitter::Node;

use super::model::{Class, Field, FieldDefault, Flag, ModelKey, Relation};
use super::values::{boolean, raw, sql_literal, text};
use crate::deploy::python_calls::Call;

const SELF_REFERENCE: &str = "self";
const USER_MODEL_SETTING: &str = "AUTH_USER_MODEL";
const AUTH_APP: &str = "auth";
const USER_MODEL: &str = "user";
const APP_SEPARATOR: char = '.';
const DB_VALUE_WRAPPER: &str = "Value";
const FLAG_KEYWORDS: &[(&str, Flag)] = &[
    ("null", Flag::Null),
    ("blank", Flag::Blank),
    ("unique", Flag::Unique),
    ("primary_key", Flag::PrimaryKey),
    ("auto_now", Flag::AutoNow),
    ("auto_now_add", Flag::AutoNow),
];
const CLASSES: &[(&str, Class)] = &[
    ("AutoField", Class::Auto),
    ("BigAutoField", Class::BigAuto),
    ("SmallAutoField", Class::SmallAuto),
    ("CharField", Class::Char),
    ("SlugField", Class::Slug),
    ("EmailField", Class::Email),
    ("URLField", Class::Url),
    ("FileField", Class::File),
    ("ImageField", Class::File),
    ("FilePathField", Class::File),
    ("TextField", Class::Text),
    ("BinaryField", Class::Binary),
    ("IntegerField", Class::Integer),
    ("BigIntegerField", Class::BigInteger),
    ("SmallIntegerField", Class::SmallInteger),
    ("PositiveIntegerField", Class::PositiveInteger),
    ("PositiveBigIntegerField", Class::PositiveBigInteger),
    ("PositiveSmallIntegerField", Class::PositiveSmallInteger),
    ("BooleanField", Class::Boolean),
    ("NullBooleanField", Class::Boolean),
    ("DateTimeField", Class::DateTime),
    ("DateField", Class::Date),
    ("TimeField", Class::Time),
    ("FloatField", Class::Float),
    ("DecimalField", Class::Decimal),
    ("UUIDField", Class::Uuid),
    ("JSONField", Class::Json),
    ("GenericIPAddressField", Class::Inet),
    ("IPAddressField", Class::Inet),
    ("DurationField", Class::Duration),
    ("ForeignKey", Class::ForeignKey),
    ("OneToOneField", Class::OneToOne),
    ("ManyToManyField", Class::ManyToMany),
];

#[derive(Debug, Clone, Copy)]
pub struct Owner<'context> {
    pub app: &'context str,
    pub model: &'context str,
}

pub fn class_of(method: &str) -> Class {
    CLASSES
        .iter()
        .find(|(name, _)| *name == method)
        .map_or(Class::Other, |(_, class)| *class)
}

pub fn field(name: &str, node: Node<'_>, source: &[u8], owner: Owner<'_>) -> Option<Field> {
    let call = Call::parse(node, source)?;
    let mut field = Field::new(name, class_of(call.method));
    for (keyword, flag) in FLAG_KEYWORDS {
        if call.keyword_is_true(keyword) {
            field.flags.insert(*flag);
        }
    }
    match call.keyword("db_index").and_then(boolean) {
        Some(true) => {
            field.flags.insert(Flag::IndexOn);
        }
        Some(false) => {
            field.flags.insert(Flag::IndexOff);
        }
        None => {}
    }
    if call.keyword("db_constraint").and_then(boolean) == Some(false) {
        field.flags.insert(Flag::NoConstraint);
    }
    field.max_length = call.keyword("max_length").map(|value| raw(value, source));
    field.max_digits = call.keyword("max_digits").map(|value| raw(value, source));
    field.decimal_places = call.keyword("decimal_places").map(|value| raw(value, source));
    field.db_column = call.keyword("db_column").and_then(|value| text(value, source));
    field.default = call.keyword("default").map_or(FieldDefault::Unset, |value| {
        sql_literal(value, source).map_or(FieldDefault::Null, FieldDefault::Literal)
    });
    field.db_default = call
        .keyword("db_default")
        .and_then(|value| database_default(value, source));
    field.relation = relation(&call, field.class, owner);
    Some(field)
}

fn database_default(node: Node<'_>, source: &[u8]) -> Option<String> {
    let wrapped = Call::parse(node, source)
        .filter(|call| call.method == DB_VALUE_WRAPPER)
        .and_then(|call| call.positional(0));
    sql_literal(wrapped.unwrap_or(node), source)
}

fn relation(call: &Call<'_>, class: Class, owner: Owner<'_>) -> Option<Relation> {
    if !matches!(class, Class::ForeignKey | Class::OneToOne | Class::ManyToMany) {
        return None;
    }
    let target = call.keyword("to").or_else(|| call.positional(0))?;
    Some(Relation {
        to: model_reference(target, call.source, owner)?,
        to_field: call
            .keyword("to_field")
            .and_then(|value| text(value, call.source)),
        through: call
            .keyword("through")
            .and_then(|value| model_reference(value, call.source, owner)),
        db_table: call
            .keyword("db_table")
            .and_then(|value| text(value, call.source)),
    })
}

pub fn model_reference(node: Node<'_>, source: &[u8], owner: Owner<'_>) -> Option<ModelKey> {
    if raw(node, source).ends_with(USER_MODEL_SETTING) {
        return Some(ModelKey::new(AUTH_APP, USER_MODEL));
    }
    let reference = text(node, source)?;
    if reference == SELF_REFERENCE {
        return Some(ModelKey::new(owner.app, owner.model));
    }
    Some(match reference.split_once(APP_SEPARATOR) {
        Some((app, model)) => ModelKey::new(app, model),
        None => ModelKey::new(owner.app, &reference),
    })
}
