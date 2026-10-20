use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone)]
pub struct Match {
    pub tokens: usize,
    pub marks: Vec<usize>,
}

#[derive(Debug)]
struct Candidate {
    tokens: usize,
    marks: BTreeSet<usize>,
}

enum Outcome {
    Embedded,
    Superseded,
    Joined,
    Unrelated,
}

#[derive(Debug)]
pub struct MatchCollector<'code> {
    code: &'code [u32],
    tile: usize,
    match_tree: BTreeMap<usize, Vec<Candidate>>,
    token_match_sets: HashMap<usize, BTreeSet<usize>>,
}

impl<'code> MatchCollector<'code> {
    pub fn new(code: &'code [u32], tile: usize) -> Self {
        Self {
            code,
            tile,
            match_tree: BTreeMap::new(),
            token_match_sets: HashMap::new(),
        }
    }

    pub fn collect(&mut self, marks: &[usize]) {
        let mut first = 0_usize;
        while first.saturating_add(1) < marks.len() {
            let later = marks.get(first.saturating_add(1)..).unwrap_or_default();
            let skipped = marks
                .get(first)
                .map_or(0, |&mark1| self.pair_with_later(mark1, later));
            first = first.saturating_add(skipped).saturating_add(1);
        }
    }

    fn pair_with_later(&mut self, mark1: usize, later: &[usize]) -> usize {
        let mut skipped = 0_usize;
        for &mark2 in later {
            if mark2.saturating_sub(mark1) < self.tile {
                skipped = skipped.saturating_add(1);
                continue;
            }
            self.consider(mark1, mark2);
        }
        skipped
    }

    pub fn into_matches(self) -> Vec<Match> {
        let mut matches: Vec<Match> = self
            .match_tree
            .into_values()
            .flatten()
            .map(|candidate| Match {
                tokens: candidate.tokens,
                marks: candidate.marks.into_iter().collect(),
            })
            .collect();
        matches.sort_by(|left, right| {
            right
                .tokens
                .cmp(&left.tokens)
                .then_with(|| left.marks.first().cmp(&right.marks.first()))
        });
        matches
    }

    fn consider(&mut self, mark1: usize, mark2: usize) {
        if self.has_previous_dupe(mark1, mark2) {
            return;
        }
        let dupes = self.count_duplicate_tokens(mark1, mark2);
        if dupes < self.tile || mark1.saturating_add(dupes) > mark2 {
            return;
        }
        self.report_match(mark1, mark2, dupes);
    }

    fn identifier(&self, index: usize) -> u32 {
        self.code.get(index).copied().unwrap_or_default()
    }

    fn match_ended(&self, first: usize, second: usize) -> bool {
        let (left, right) = (self.identifier(first), self.identifier(second));
        left != right || left == 0 || right == 0
    }

    fn has_previous_dupe(&self, mark1: usize, mark2: usize) -> bool {
        mark1 != 0 && !self.match_ended(mark1.saturating_sub(1), mark2.saturating_sub(1))
    }

    fn count_duplicate_tokens(&self, mark1: usize, mark2: usize) -> usize {
        (0..=self.code.len())
            .find(|&offset| self.match_ended(mark1.saturating_add(offset), mark2.saturating_add(offset)))
            .unwrap_or_default()
    }

    fn report_match(&mut self, mark1: usize, mark2: usize, dupes: usize) {
        let known = self.token_match_sets.entry(mark1).or_default();
        if known.contains(&mark2) {
            return;
        }
        let lowest_key = known.first().map_or(mark1, |&lowest| lowest.min(mark1));
        let candidates = self.match_tree.entry(lowest_key).or_default();
        let mut index = 0;
        let mut joined = None;
        while let Some(candidate) = candidates.get_mut(index) {
            match classify(candidate, mark1, mark2, dupes) {
                Outcome::Embedded => return,
                Outcome::Superseded => {
                    candidates.remove(index);
                }
                Outcome::Joined => {
                    joined = Some(candidate.marks.iter().copied().collect::<Vec<_>>());
                    candidate.marks.insert(mark2);
                    break;
                }
                Outcome::Unrelated => index = index.saturating_add(1),
            }
        }
        if let Some(others) = joined {
            for other in others {
                register(&mut self.token_match_sets, other, mark2);
            }
            return;
        }
        candidates.push(Candidate {
            tokens: dupes,
            marks: BTreeSet::from([mark1, mark2]),
        });
        register(&mut self.token_match_sets, mark1, mark2);
    }
}

fn register(token_match_sets: &mut HashMap<usize, BTreeSet<usize>>, first: usize, second: usize) {
    token_match_sets.entry(first).or_default().insert(second);
    token_match_sets.entry(second).or_default().insert(first);
}

fn classify(candidate: &Candidate, mark1: usize, mark2: usize, dupes: usize) -> Outcome {
    let new_end = mark2.saturating_add(dupes);
    for &other in candidate.marks.iter().filter(|&&other| other != mark1) {
        let other_end = other.saturating_add(candidate.tokens);
        if other < mark2 && other_end >= new_end {
            return Outcome::Embedded;
        }
        if mark2 < other && new_end >= other_end {
            return Outcome::Superseded;
        }
        if dupes == candidate.tokens {
            return Outcome::Joined;
        }
    }
    Outcome::Unrelated
}
