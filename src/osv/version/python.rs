use std::cmp::Ordering;
use std::sync::OnceLock;

use regex::{Captures, Regex};

use super::{compare_numbers, compare_semver};

const PEP_440: &str = r"^v?(?:(?P<epoch>[0-9]+)!)?(?P<release>[0-9]+(?:\.[0-9]+)*)(?:[-_.]?(?P<pre_l>alpha|a|beta|b|preview|pre|c|rc)[-_.]?(?P<pre_n>[0-9]+)?)?(?:-(?P<post_n1>[0-9]+)|[-_.]?(?P<post_l>post|rev|r)[-_.]?(?P<post_n2>[0-9]+)?)?(?:[-_.]?(?P<dev_l>dev)[-_.]?(?P<dev_n>[0-9]+)?)?(?:\+[a-z0-9]+(?:[-_.][a-z0-9]+)*)?$";
const RELEASE_SEPARATOR: char = '.';
const ALPHA_RANK: i64 = 0;
const BETA_RANK: i64 = 1;
const CANDIDATE_RANK: i64 = 2;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    epoch: u64,
    release: Vec<u64>,
    pre: (i64, u64),
    post: i64,
    dev: i64,
}

fn pattern() -> Option<&'static Regex> {
    static PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(PEP_440).ok()).as_ref()
}

pub fn compare(left: &str, right: &str) -> Ordering {
    match (key(left), key(right)) {
        (Some(left), Some(right)) => left
            .epoch
            .cmp(&right.epoch)
            .then_with(|| compare_numbers(&left.release, &right.release))
            .then_with(|| (left.pre, left.post, left.dev).cmp(&(right.pre, right.post, right.dev))),
        _ => compare_semver(left, right),
    }
}

fn number(captures: &Captures<'_>, group: &str) -> Option<u64> {
    captures
        .name(group)
        .map(|found| found.as_str().parse().unwrap_or_default())
}

fn signed(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn key(version: &str) -> Option<Key> {
    let lowered = version.trim().to_lowercase();
    let captures = pattern()?.captures(&lowered)?;
    let release = captures
        .name("release")?
        .as_str()
        .split(RELEASE_SEPARATOR)
        .map(|part| part.parse().unwrap_or_default())
        .collect();
    let pre_rank = captures.name("pre_l").map(|letter| match letter.as_str() {
        "a" | "alpha" => ALPHA_RANK,
        "b" | "beta" => BETA_RANK,
        _ => CANDIDATE_RANK,
    });
    let has_post = captures.name("post_n1").is_some() || captures.name("post_l").is_some();
    let post = number(&captures, "post_n1")
        .or_else(|| has_post.then(|| number(&captures, "post_n2").unwrap_or_default()));
    let dev = captures
        .name("dev_l")
        .map(|_| number(&captures, "dev_n").unwrap_or_default());
    let pre = match (pre_rank, post, dev) {
        (Some(rank), _, _) => (rank, number(&captures, "pre_n").unwrap_or_default()),
        (None, None, Some(_)) => (i64::MIN, 0),
        (None, _, _) => (i64::MAX, 0),
    };
    Some(Key {
        epoch: number(&captures, "epoch").unwrap_or_default(),
        release,
        pre,
        post: post.map_or(i64::MIN, signed),
        dev: dev.map_or(i64::MAX, signed),
    })
}
