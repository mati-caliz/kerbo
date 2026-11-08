use tree_sitter::Node;

use super::model::{Class, Field, FieldDefault, Flag, ModelKey, Relation};
use super::settings::Settings;
use super::values::{boolean, quoted_literal, raw, sql_literal, text};
use crate::deploy::python_calls::Call;

const SELF_REFERENCE: &str = "self";
const USER_MODEL_SETTING: &str = "AUTH_USER_MODEL";
const APP_SEPARATOR: char = '.';
const DB_VALUE_WRAPPER: &str = "Value";
const ARRAY_ELEMENT: &str = "base_field";
const NONE: &str = "none";
const DECIMAL_CONSTRUCTOR: &str = "Decimal";
const JSON_CAST: &str = "::jsonb";
const EMPTY_OBJECT: &str = "{}";
const EMPTY_ARRAY: &str = "[]";
const OBJECT_DEFAULTS: &[&str] = &["dict", "dictionary"];
const ARRAY_DEFAULTS: &[&str] = &["list", "tuple"];
const ARRAY_LITERALS: &[&str] = &["list", "tuple"];
const OBJECT_LITERAL: &str = "dictionary";
pub const NOW_DEFAULT: &str = "'2000-01-01 00:00:00+00:00'::timestamptz";
const TODAY_DEFAULT: &str = "'2000-01-01'::date";
const UUID_DEFAULT: &str = "'00000000000000000000000000000000'::uuid";
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
    ("HStoreField", Class::HStore),
    ("SearchVectorField", Class::SearchVector),
    ("ArrayField", Class::Array),
    ("CountryField", Class::Char),
    ("PhoneNumberField", Class::Char),
    ("PossiblePhoneNumberField", Class::Char),
    ("WebhookURLField", Class::Url),
    ("SanitizedJSONField", Class::Json),
    ("MeasurementField", Class::Float),
    ("ForeignKey", Class::ForeignKey),
    ("OneToOneField", Class::OneToOne),
    ("ManyToManyField", Class::ManyToMany),
];

#[derive(Debug, Clone, Copy)]
pub struct Owner<'context> {
    pub app: &'context str,
    pub model: &'context str,
    pub settings: &'context Settings,
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
    let number = |keyword: &str| {
        call.keyword(keyword)
            .map(|value| owner.settings.number(&raw(value, source)).to_owned())
    };
    field.max_length = number("max_length");
    field.max_digits = number("max_digits");
    field.decimal_places = number("decimal_places");
    field.db_column = call.keyword("db_column").and_then(|value| text(value, source));
    field.default = call
        .keyword("default")
        .map_or(FieldDefault::Unset, |value| default(value, source, field.class));
    field.db_default = call
        .keyword("db_default")
        .and_then(|value| database_default(value, source));
    field.relation = relation(&call, field.class, owner);
    field.element = call
        .keyword(ARRAY_ELEMENT)
        .or_else(|| call.positional(0))
        .filter(|_| field.class == Class::Array)
        .and_then(|element| self::field(name, element, source, owner))
        .map(Box::new);
    Some(field)
}

fn default(node: Node<'_>, source: &[u8], class: Class) -> FieldDefault {
    if node.kind() == NONE {
        return FieldDefault::Null;
    }
    typed_default(node, source, class)
        .or_else(|| sql_literal(node, source))
        .map_or(FieldDefault::Null, FieldDefault::Literal)
}

fn typed_default(node: Node<'_>, source: &[u8], class: Class) -> Option<String> {
    if let Some(placeholder) = placeholder_default(class) {
        return Some(placeholder.to_owned());
    }
    if class == Class::Json {
        return empty_json(node, source).map(|json| format!("{}{JSON_CAST}", quoted_literal(json)));
    }
    if class == Class::Decimal {
        return Call::parse(node, source)
            .filter(|call| call.method == DECIMAL_CONSTRUCTOR)
            .and_then(|call| call.positional(0))
            .and_then(|value| text(value, source));
    }
    None
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
        return Some(owner.settings.user_model.clone());
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
