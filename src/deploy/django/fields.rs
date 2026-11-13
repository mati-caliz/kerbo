use std::path::Path;

use tree_sitter::Node;

use super::columns::own_type;
use super::constants::Constants;
use super::dependencies::swappable_reference;
use super::enum_members::member_literal;
use super::field_defaults::{DB_VALUE_WRAPPER, EMPTY_POSTGRES_ARRAY, database_default, default};
use super::model::{Class, Field, FieldDefault, Flag, ModelKey, Relation};
use super::settings::Settings;
use super::truth::truth;
use super::values::{boolean, raw, text};
use crate::deploy::python_calls::Call;

const SELF_REFERENCE: &str = "self";
const APP_SEPARATOR: char = '.';
const OUTPUT_FIELD: &str = "output_field";
const ARRAY_ELEMENT: &str = "base_field";
const NONE: &str = "none";
const CONDITIONAL: &str = "conditional_expression";
const CONDITIONAL_BODY: u32 = 0;
const CONDITIONAL_TEST: u32 = 1;
const CONDITIONAL_ALTERNATIVE: u32 = 2;
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
    ("BitField", Class::BigInteger),
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
    ("SanitizedJSONField", Class::SanitizedJson),
    ("MeasurementField", Class::Float),
    ("ForeignKey", Class::ForeignKey),
    ("ParentalKey", Class::ForeignKey),
    ("ParentalManyToManyField", Class::ManyToMany),
    ("StreamField", Class::Json),
    ("TaggableManager", Class::ManyToMany),
    ("ClusterTaggableManager", Class::ManyToMany),
    ("OneToOneField", Class::OneToOne),
    ("ManyToManyField", Class::ManyToMany),
];

#[derive(Debug, Clone, Copy)]
pub struct Owner<'context> {
    pub app: &'context str,
    pub model: &'context str,
    pub settings: &'context Settings,
    pub constants: Constants<'context>,
    pub apps_root: Option<&'context Path>,
}

pub fn class_of(method: &str) -> Class {
    CLASSES
        .iter()
        .find(|(name, _)| *name == method)
        .map_or(Class::Other, |(_, class)| *class)
}

pub fn field<'tree>(
    name: &str,
    node: Node<'tree>,
    source: &'tree [u8],
    owner: Owner<'tree>,
) -> Option<Field> {
    let call = Call::parse(node, source)?;
    let classes = &owner.settings.field_classes;
    let qualified = format!("{}{APP_SEPARATOR}{}", call.receiver, call.method);
    let class = classes
        .get(&qualified)
        .copied()
        .or_else(|| Some(class_of(call.method)).filter(|known| *known != Class::Other))
        .or_else(|| classes.get(call.method).copied())
        .unwrap_or(Class::Other);
    let mut field = Field::new(name, class);
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
            .filter(|value| value.kind() != NONE)
            .map(|value| owner.settings.number(&raw(value, source)).to_owned())
    };
    field.max_length = number("max_length");
    field.max_digits = number("max_digits");
    field.decimal_places = number("decimal_places");
    field.size = number("size");
    field.db_column = call.keyword("db_column").and_then(|value| text(value, source));
    field.db_comment = call.keyword("db_comment").and_then(|value| text(value, source));
    field.db_collation = call.keyword("db_collation").and_then(|value| text(value, source));
    let enum_member = |value: Node<'_>| member_literal(constant_value(value, owner), source, owner.apps_root);
    field.default = call.keyword("default").map_or(FieldDefault::Unset, |value| {
        enum_member(value).map_or_else(
            || default(constant_value(value, owner), source, field.class),
            FieldDefault::Literal,
        )
    });
    if call
        .keyword("db_default")
        .and_then(|value| Call::parse(value, source))
        .is_some_and(|value| value.method == DB_VALUE_WRAPPER && value.keyword(OUTPUT_FIELD).is_some())
    {
        field.flags.insert(Flag::TypedDatabaseDefault);
    }
    field.db_default = call.keyword("db_default").and_then(|value| {
        enum_member(value).or_else(|| database_default(constant_value(value, owner), source, field.class))
    });
    field.relation = relation(&call, field.class, owner);
    field.element = call
        .keyword(ARRAY_ELEMENT)
        .or_else(|| call.positional(0))
        .filter(|_| field.class == Class::Array)
        .and_then(|element| self::field(name, element, source, owner))
        .map(Box::new);
    if field.db_default.as_deref() == Some(EMPTY_POSTGRES_ARRAY) {
        field.db_default = Some(format!("{EMPTY_POSTGRES_ARRAY}::{}", own_type(&field)));
    }
    Some(field)
}

fn constant_value<'tree>(node: Node<'tree>, owner: Owner<'tree>) -> Node<'tree> {
    let resolved = owner.constants.resolve(node);
    if resolved.kind() != CONDITIONAL {
        return resolved;
    }
    let branch = resolved
        .named_child(CONDITIONAL_TEST)
        .and_then(|condition| {
            truth(
                owner.constants.resolve(condition),
                owner.constants.source,
                owner.settings,
            )
        })
        .and_then(|holds| {
            resolved.named_child(if holds {
                CONDITIONAL_BODY
            } else {
                CONDITIONAL_ALTERNATIVE
            })
        });
    branch.map_or(resolved, |chosen| constant_value(chosen, owner))
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
    if let Some(swappable) = swappable_reference(node, source, owner.settings) {
        return Some(swappable);
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
