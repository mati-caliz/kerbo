use std::cmp::Ordering;

const SEPARATORS: [char; 2] = ['.', '-'];
const QUALIFIER_ORDER: [&str; 7] = ["alpha", "beta", "milestone", "rc", "snapshot", "", "sp"];
const RELEASE_ALIASES: [&str; 3] = ["ga", "final", "release"];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Number(u64),
    Qualifier(String),
}

pub fn compare(left: &str, right: &str) -> Ordering {
    let left = items(left);
    let right = items(right);
    let length = left.len().max(right.len());
    (0..length)
        .map(|index| compare_items(left.get(index), right.get(index)))
        .find(|order| *order != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

fn items(version: &str) -> Vec<Item> {
    let lowered = version.trim().to_lowercase();
    let mut found = Vec::new();
    for part in lowered.split(SEPARATORS) {
        let mut current = String::new();
        let mut digits = None;
        for character in part.chars() {
            let is_digit = character.is_ascii_digit();
            if digits.is_some_and(|previous| previous != is_digit) {
                found.push(item(&current, digits == Some(true)));
                current.clear();
            }
            digits = Some(is_digit);
            current.push(character);
        }
        found.push(item(&current, digits == Some(true)));
    }
    found
}

fn item(text: &str, numeric: bool) -> Item {
    if numeric {
        return Item::Number(text.parse().unwrap_or_default());
    }
    let canonical = match text {
        "a" => "alpha",
        "b" => "beta",
        "m" => "milestone",
        "cr" => "rc",
        alias if RELEASE_ALIASES.contains(&alias) => "",
        other => other,
    };
    Item::Qualifier(canonical.to_owned())
}

fn qualifier_rank(qualifier: &str) -> (usize, &str) {
    QUALIFIER_ORDER
        .iter()
        .position(|known| *known == qualifier)
        .map_or((QUALIFIER_ORDER.len(), qualifier), |rank| (rank, ""))
}

fn compare_items(left: Option<&Item>, right: Option<&Item>) -> Ordering {
    match (left, right) {
        (Some(Item::Number(left)), Some(Item::Number(right))) => left.cmp(right),
        (Some(Item::Number(_)), Some(Item::Qualifier(_))) => Ordering::Greater,
        (Some(Item::Qualifier(_)), Some(Item::Number(_))) => Ordering::Less,
        (Some(Item::Qualifier(left)), Some(Item::Qualifier(right))) => {
            qualifier_rank(left).cmp(&qualifier_rank(right))
        }
        (Some(Item::Number(value)), None) => value.cmp(&0),
        (None, Some(Item::Number(value))) => 0.cmp(value),
        (Some(Item::Qualifier(qualifier)), None) => qualifier_rank(qualifier).cmp(&qualifier_rank("")),
        (None, Some(Item::Qualifier(qualifier))) => qualifier_rank("").cmp(&qualifier_rank(qualifier)),
        (None, None) => Ordering::Equal,
    }
}
