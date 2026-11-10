use tree_sitter::Node;

use super::constants::Constants;
use super::dependencies::swappable_reference;
use super::model::{Class, Field, FieldDefault, Flag, ModelKey, Relation};
use super::settings::Settings;
use super::truth::truth;
use super::values::{boolean, quoted_literal, raw, sql_literal, text};
use crate::deploy::python_calls::Call;

const SELF_REFERENCE: &str = "self";
const APP_SEPARATOR: char = '.';
const DB_VALUE_WRAPPER: &str = "Value";
const ARRAY_ELEMENT: &str = "base_field";
const NONE: &str = "none";
const CONDITIONAL: &str = "conditional_expression";
const CONDITIONAL_BODY: u32 = 0;
const CONDITIONAL_TEST: u32 = 1;
const CONDITIONAL_ALTERNATIVE: u32 = 2;
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
    field.default = call.keyword("default").map_or(FieldDefault::Unset, |value| {
        default(constant_value(value, owner), source, field.class)
    });
    field.db_default = call
        .keyword("db_default")
        .and_then(|value| database_default(constant_value(value, owner), source, field.class));
    field.relation = relation(&call, field.class, owner);
    field.element = call
        .keyword(ARRAY_ELEMENT)
        .or_else(|| call.positional(0))
        .filter(|_| field.class == Class::Array)
        .and_then(|element| self::field(name, element, source, owner))
        .map(Box::new);
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

fn default(node: Node<'_>, source: &[u8], class: Class) -> FieldDefault {
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

fn database_default(node: Node<'_>, source: &[u8], class: Class) -> Option<String> {
    let wrapped = Call::parse(node, source)
        .filter(|call| call.method == DB_VALUE_WRAPPER)
        .and_then(|call| call.positional(0));
    let value = wrapped.unwrap_or(node);
    let typed = if class == Class::Json {
        json_default(value, source)
    } else if class == Class::Decimal {
        decimal_argument(value, source)
    } else {
        None
    };
    typed.or_else(|| sql_literal(value, source))
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
