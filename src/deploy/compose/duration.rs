use std::time::Duration;

const UNITS: &[(&str, u64)] = &[
    ("ns", 1),
    ("us", 1_000),
    ("µs", 1_000),
    ("ms", 1_000_000),
    ("s", 1_000_000_000),
    ("m", 60_000_000_000),
    ("h", 3_600_000_000_000),
];

pub fn parse(text: &str) -> Option<Duration> {
    let mut rest = text.trim();
    if rest.is_empty() {
        return None;
    }
    let mut nanoseconds = 0_u64;
    while !rest.is_empty() {
        let digits = rest
            .find(|character: char| !(character.is_ascii_digit() || character == '.'))
            .unwrap_or(rest.len());
        let (number, after) = rest.split_at_checked(digits)?;
        let amount: u64 = number.parse().ok()?;
        let (unit, factor) = UNITS
            .iter()
            .filter(|(unit, _)| after.starts_with(unit))
            .max_by_key(|(unit, _)| unit.len())?;
        nanoseconds = nanoseconds.checked_add(amount.checked_mul(*factor)?)?;
        rest = after.get(unit.len()..)?;
    }
    Some(Duration::from_nanos(nanoseconds))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::parse;

    #[test]
    fn reads_compound_go_durations() {
        assert_eq!(parse("1m30s"), Some(Duration::from_secs(90)));
        assert_eq!(parse("500ms"), Some(Duration::from_millis(500)));
        assert_eq!(parse("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(parse("${INTERVAL:-10s}"), None);
        assert_eq!(parse("10"), None);
        assert_eq!(parse(""), None);
    }
}
