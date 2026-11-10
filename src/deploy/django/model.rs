use std::collections::BTreeSet;

use super::condition::Condition;
use super::expression::Expression;
use super::names::truncated_table;

const COLUMN_SUFFIX: &str = "_id";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    Null,
    Blank,
    Unique,
    PrimaryKey,
    IndexOn,
    IndexOff,
    NoConstraint,
    AutoNow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Auto,
    BigAuto,
    SmallAuto,
    Char,
    Slug,
    Email,
    Url,
    File,
    Text,
    Binary,
    Integer,
    BigInteger,
    SmallInteger,
    PositiveInteger,
    PositiveBigInteger,
    PositiveSmallInteger,
    Boolean,
    DateTime,
    Date,
    Time,
    Float,
    Decimal,
    Uuid,
    Json,
    SanitizedJson,
    Inet,
    Duration,
    HStore,
    SearchVector,
    Array,
    ForeignKey,
    OneToOne,
    ManyToMany,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldDefault {
    Unset,
    Null,
    Literal(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelKey {
    pub app: String,
    pub model: String,
}

impl ModelKey {
    pub fn new(app: &str, model: &str) -> Self {
        Self {
            app: app.to_owned(),
            model: model.to_lowercase(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub to: ModelKey,
    pub to_field: Option<String>,
    pub through: Option<ModelKey>,
    pub db_table: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub class: Class,
    pub flags: BTreeSet<Flag>,
    pub max_length: Option<String>,
    pub max_digits: Option<String>,
    pub decimal_places: Option<String>,
    pub size: Option<String>,
    pub default: FieldDefault,
    pub db_default: Option<String>,
    pub db_column: Option<String>,
    pub relation: Option<Relation>,
    pub element: Option<Box<Self>>,
}

impl Field {
    pub fn new(name: &str, class: Class) -> Self {
        Self {
            name: name.to_owned(),
            class,
            flags: BTreeSet::new(),
            max_length: None,
            max_digits: None,
            decimal_places: None,
            size: None,
            default: FieldDefault::Unset,
            db_default: None,
            db_column: None,
            relation: None,
            element: None,
        }
    }

    pub fn has(&self, flag: Flag) -> bool {
        self.flags.contains(&flag)
    }

    pub fn null(&self) -> bool {
        self.has(Flag::Null)
    }

    pub fn primary_key(&self) -> bool {
        self.has(Flag::PrimaryKey)
    }

    pub fn unique(&self) -> bool {
        self.has(Flag::Unique) || self.primary_key() || self.class == Class::OneToOne
    }

    pub fn db_index(&self) -> bool {
        self.has(Flag::IndexOn) || (self.indexed_by_default() && !self.has(Flag::IndexOff))
    }

    const fn indexed_by_default(&self) -> bool {
        self.is_foreign_key() || matches!(self.class, Class::Slug)
    }

    pub fn db_constraint(&self) -> bool {
        !self.has(Flag::NoConstraint)
    }

    pub const fn is_foreign_key(&self) -> bool {
        matches!(self.class, Class::ForeignKey | Class::OneToOne)
    }

    pub fn is_many_to_many(&self) -> bool {
        self.class == Class::ManyToMany
    }

    pub fn column(&self) -> String {
        self.db_column.clone().unwrap_or_else(|| {
            if self.is_foreign_key() {
                format!("{}{COLUMN_SUFFIX}", self.name)
            } else {
                self.name.clone()
            }
        })
    }

    pub fn auto_through(&self) -> bool {
        self.is_many_to_many()
            && self
                .relation
                .as_ref()
                .is_some_and(|relation| relation.through.is_none())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub name: String,
    pub fields: Vec<String>,
    pub expressions: Vec<Expression>,
    pub condition: Option<Condition>,
    pub method: Option<&'static str>,
    pub opclasses: Vec<String>,
    pub storage: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintKind {
    Unique {
        fields: Vec<String>,
        expressions: Vec<Expression>,
        by_index: bool,
        condition: Option<Condition>,
        opclasses: Vec<String>,
        nulls_distinct: Option<bool>,
    },
    Check {
        condition: Condition,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub name: String,
    pub kind: ConstraintKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub key: ModelKey,
    pub db_table: Option<String>,
    pub fields: Vec<Field>,
    pub unique_together: Vec<Vec<String>>,
    pub indexes: Vec<Index>,
    pub constraints: Vec<Constraint>,
    pub managed: bool,
    pub proxy: bool,
    pub swapped_for: Option<ModelKey>,
    pub swappable_setting: Option<String>,
}

impl Model {
    pub const fn new(key: ModelKey) -> Self {
        Self {
            key,
            db_table: None,
            fields: Vec::new(),
            unique_together: Vec::new(),
            indexes: Vec::new(),
            constraints: Vec::new(),
            managed: true,
            proxy: false,
            swapped_for: None,
            swappable_setting: None,
        }
    }

    pub fn table(&self) -> String {
        self.db_table
            .clone()
            .unwrap_or_else(|| truncated_table(&format!("{}_{}", self.key.app, self.key.model)))
    }

    pub const fn emits_sql(&self) -> bool {
        self.managed && !self.proxy && self.swapped_for.is_none()
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn primary_key(&self) -> Option<&Field> {
        self.fields
            .iter()
            .find(|field| field.primary_key())
            .or_else(|| self.fields.first())
    }

    pub fn column_of(&self, name: &str) -> String {
        self.field(name).map_or_else(|| name.to_owned(), Field::column)
    }

    pub fn local_fields(&self) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter(|field| !field.is_many_to_many())
    }
}
