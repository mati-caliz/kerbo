use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::go_syntax::SyntaxNode;
use super::suffix_tree::{Repeat, SuffixTree};
use super::{CloneGroup, FNV_OFFSET, Placement, fnv};

const TERMINATOR: i32 = -1;

#[derive(Debug, Clone, Copy)]
struct Pooled {
    file: usize,
    node: SyntaxNode,
}

struct Units {
    hash: u64,
    fragments: Vec<Vec<usize>>,
}

pub fn clone_groups(files: Vec<Vec<SyntaxNode>>, threshold: usize) -> Vec<CloneGroup> {
    let data: Vec<Pooled> = files
        .into_iter()
        .enumerate()
        .flat_map(|(file, nodes)| nodes.into_iter().map(move |node| Pooled { file, node }))
        .collect();
    let mut tree = SuffixTree::new();
    for pooled in &data {
        tree.push(pooled.node.kind);
    }
    tree.push(TERMINATOR);
    let mut by_hash: BTreeMap<u64, Vec<Vec<usize>>> = BTreeMap::new();
    tree.for_each_repeat(threshold, |repeat| {
        if let Some(units) = syntax_units(&data, repeat, threshold) {
            by_hash.entry(units.hash).or_default().extend(units.fragments);
        }
    });
    by_hash
        .into_iter()
        .filter_map(|(hash, fragments)| clone_group(&data, hash, fragments))
        .collect()
}

fn clone_group(data: &[Pooled], hash: u64, fragments: Vec<Vec<usize>>) -> Option<CloneGroup> {
    let mut seen = HashSet::new();
    let unique: Vec<Vec<usize>> = fragments
        .into_iter()
        .filter(|fragment| {
            fragment
                .first()
                .and_then(|&first| data.get(first))
                .is_some_and(|pooled| seen.insert((pooled.file, pooled.node.start)))
        })
        .collect();
    if unique.len() < 2 {
        return None;
    }
    let size = unique
        .first()?
        .iter()
        .filter_map(|&index| data.get(index))
        .map(|pooled| pooled.node.owns.saturating_add(1))
        .sum();
    let copies = unique
        .iter()
        .filter_map(|fragment| {
            let first = data.get(*fragment.first()?)?;
            let last = data.get(*fragment.last()?)?;
            Some(Placement {
                file: first.file,
                line: first.node.line,
                end_line: last.node.end_line,
            })
        })
        .collect();
    Some(CloneGroup {
        size,
        content: hash,
        copies,
    })
}

fn node_at(data: &[Pooled], index: usize) -> Option<SyntaxNode> {
    data.get(index).map(|pooled| pooled.node)
}

fn syntax_units(data: &[Pooled], repeat: &Repeat, threshold: usize) -> Option<Units> {
    let &first_position = repeat.positions.first()?;
    let first_sequence = data.get(first_position..first_position.saturating_add(repeat.length))?;
    let mut indexes = unit_indexes(first_sequence, threshold);
    if let Some(&last) = indexes.last() {
        let owns = first_sequence.get(last).map(|pooled| pooled.node.owns);
        let differs = repeat
            .positions
            .iter()
            .skip(1)
            .any(|&position| node_at(data, position.saturating_add(last)).map(|node| node.owns) != owns);
        if differs {
            indexes.pop();
        }
    }
    if indexes.is_empty()
        || is_cyclic(&indexes, first_sequence)
        || spans_multiple_files(&indexes, first_sequence)
    {
        return None;
    }
    let fragments = repeat
        .positions
        .iter()
        .map(|&position| {
            indexes
                .iter()
                .map(|&index| position.saturating_add(index))
                .collect()
        })
        .collect();
    let &first_index = indexes.first()?;
    let &last_index = indexes.last()?;
    let hashed_end = last_index.saturating_add(first_sequence.get(last_index)?.node.owns);
    let kinds = first_sequence
        .get(first_index..hashed_end)
        .unwrap_or_default()
        .iter()
        .map(|pooled| {
            u64::from(
                pooled
                    .node
                    .kind
                    .to_le_bytes()
                    .first()
                    .copied()
                    .unwrap_or_default(),
            )
        });
    Some(Units {
        hash: fnv(FNV_OFFSET, kinds),
        fragments,
    })
}

fn unit_indexes(sequence: &[Pooled], threshold: usize) -> Vec<usize> {
    let mut indexes = Vec::new();
    let mut split = false;
    let mut index = 0;
    while let Some(pooled) = sequence.get(index) {
        let owns = pooled.node.owns;
        if owns >= sequence.len().saturating_sub(index) {
            index = index.saturating_add(1);
            split = true;
            continue;
        }
        if owns.saturating_add(1) < threshold {
            split = true;
        } else {
            if split {
                indexes.clear();
                split = false;
            }
            indexes.push(index);
        }
        index = index.saturating_add(owns).saturating_add(1);
    }
    indexes
}

fn is_cyclic(indexes: &[usize], nodes: &[Pooled]) -> bool {
    let count = indexes.len();
    if count <= 1 {
        return false;
    }
    let sequence = Sequence { indexes, nodes };
    let mut alternatives: BTreeSet<usize> = (1..=count / 2)
        .filter(|&step| count.is_multiple_of(step))
        .collect();
    let first = indexes.first().copied().unwrap_or_default();
    let limit = indexes.get(count / 2).copied().unwrap_or_default();
    for offset in 0..limit {
        let start = node_at_slice(nodes, offset.saturating_add(first));
        let mut surviving = BTreeSet::new();
        for &step in &alternatives {
            match sequence.alternative(offset, step, start) {
                Alternative::Cyclic => return true,
                Alternative::Holds => {
                    surviving.insert(step);
                }
                Alternative::Broken => {}
            }
        }
        alternatives = surviving;
        if alternatives.is_empty() {
            return false;
        }
    }
    true
}

enum Alternative {
    Holds,
    Broken,
    Cyclic,
}

struct Sequence<'data> {
    indexes: &'data [usize],
    nodes: &'data [Pooled],
}

impl Sequence<'_> {
    fn alternative(&self, offset: usize, step: usize, start: Option<SyntaxNode>) -> Alternative {
        let start_index = self.indexes.get(step).copied().unwrap_or_default();
        for &unit in self.indexes.iter().skip(step).step_by(step) {
            let index = offset.saturating_add(unit);
            match node_at_slice(self.nodes, index) {
                Some(alternative) if start.is_some_and(|first| same_shape(first, alternative)) => {}
                None if offset >= start_index => return Alternative::Cyclic,
                Some(_) | None => return Alternative::Broken,
            }
        }
        Alternative::Holds
    }
}

const fn same_shape(left: SyntaxNode, right: SyntaxNode) -> bool {
    left.owns == right.owns && left.kind == right.kind
}

fn node_at_slice(nodes: &[Pooled], index: usize) -> Option<SyntaxNode> {
    nodes.get(index).map(|pooled| pooled.node)
}

fn spans_multiple_files(indexes: &[usize], nodes: &[Pooled]) -> bool {
    let mut files = indexes
        .iter()
        .filter_map(|&index| nodes.get(index))
        .map(|pooled| pooled.file);
    let first = files.next();
    files.any(|file| Some(file) != first)
}
