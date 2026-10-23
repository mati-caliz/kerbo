use std::cmp::Ordering;

use super::model::{Alias, AliasComments};
use super::normalize::{FromStatement, ImportBlock, Straight};
use crate::lint::python::natural::{is_cased_uppercase, natural_compare};

const STAR: &str = "*";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MemberType {
    Constant,
    Class,
    Variable,
}

pub enum Ordered<'source> {
    Straight(Straight<'source>),
    From(FromStatement<'source>),
}

// `order_imports` de ruff con la configuración por defecto: primero los `import`, después los
// `from`, cada grupo por módulo y con los nombres de cada `from` ordenados.
pub fn order(block: ImportBlock<'_>) -> Vec<Ordered<'_>> {
    let ImportBlock {
        mut straight,
        mut from,
    } = block;
    straight.sort_by(|left, right| {
        compare_straight(left.alias, right.alias).then(left.first_index.cmp(&right.first_index))
    });
    for statement in &mut from {
        statement
            .aliases
            .sort_by(|(left, _), (right, _)| compare_members(*left, *right));
    }
    from.sort_by(|left, right| compare_from(left, right).then(left.first_index.cmp(&right.first_index)));
    straight
        .into_iter()
        .map(Ordered::Straight)
        .chain(from.into_iter().map(Ordered::From))
        .collect()
}

fn compare_names(left: &str, right: &str) -> Ordering {
    natural_compare(&left.to_lowercase(), &right.to_lowercase()).then_with(|| natural_compare(left, right))
}

fn compare_optional(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => natural_compare(left, right),
        (left, right) => left.is_some().cmp(&right.is_some()),
    }
}

// `ModuleKey` de un `import x as y`.
fn compare_straight(left: Alias<'_>, right: Alias<'_>) -> Ordering {
    compare_names(left.name, right.name).then_with(|| compare_optional(left.asname, right.asname))
}

// `ModuleKey` de un `from`: primero los relativos más lejanos, después el módulo y el primer nombre.
fn compare_from(left: &FromStatement<'_>, right: &FromStatement<'_>) -> Ordering {
    let distance = |statement: &FromStatement<'_>| {
        (
            statement.module.level == 0,
            std::cmp::Reverse(statement.module.level),
        )
    };
    distance(left)
        .cmp(&distance(right))
        .then_with(|| match (left.module.module, right.module.module) {
            (Some(left), Some(right)) => compare_names(left, right),
            (left, right) => left.is_some().cmp(&right.is_some()),
        })
        .then_with(|| compare_first_alias(&left.aliases, &right.aliases))
}

fn compare_first_alias(
    left: &[(Alias<'_>, AliasComments<'_>)],
    right: &[(Alias<'_>, AliasComments<'_>)],
) -> Ordering {
    match (left.first(), right.first()) {
        (Some((left, _)), Some((right, _))) => compare_members(*left, *right),
        (left, right) => left.is_some().cmp(&right.is_some()),
    }
}

// `MemberKey`: primero `*`, después constantes, clases y el resto, y dentro de cada grupo en orden
// natural sin distinguir mayúsculas.
fn compare_members(left: Alias<'_>, right: Alias<'_>) -> Ordering {
    (left.name != STAR)
        .cmp(&(right.name != STAR))
        .then_with(|| member_type(left.name).cmp(&member_type(right.name)))
        .then_with(|| compare_names(left.name, right.name))
        .then_with(|| compare_optional(left.asname, right.asname))
}

fn member_type(name: &str) -> MemberType {
    if name.chars().nth(1).is_some() && is_cased_uppercase(name) {
        MemberType::Constant
    } else if name.starts_with(char::is_uppercase) {
        MemberType::Class
    } else {
        MemberType::Variable
    }
}
