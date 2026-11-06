use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode {
    pub unicode: bool,
    pub unicode_sets: bool,
}

impl Mode {
    pub fn of_flags(flags: &str) -> Self {
        Self {
            unicode: flags.contains('u'),
            unicode_sets: flags.contains('v'),
        }
    }

    pub const fn is_strict(self) -> bool {
        self.unicode || self.unicode_sets
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Pattern,
    Alternative,
    Capture(Option<String>),
    Group,
    Look { behind: bool, negate: bool },
    Backreference(Reference),
    Class { negate: bool, empty: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    Number(usize),
    Name(String),
}

// Un nodo del árbol de `regexpp`, con su padre: las reglas comparan caminos hasta la raíz.
#[derive(Debug, Clone)]
pub struct RegexNode {
    pub kind: Kind,
    pub parent: Option<usize>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Default)]
pub struct RegexTree {
    pub nodes: Vec<RegexNode>,
}

impl RegexTree {
    pub fn path(&self, node: usize) -> Vec<usize> {
        let mut path = vec![node];
        let mut current = node;
        while let Some(parent) = self.nodes.get(current).and_then(|found| found.parent) {
            path.push(parent);
            current = parent;
        }
        path
    }

    // Los grupos que puede nombrar una referencia.
    pub fn resolved(&self, reference: &Reference) -> Vec<usize> {
        let captures = self.nodes.iter().enumerate().filter_map(|(index, node)| {
            if let Kind::Capture(name) = &node.kind {
                Some((index, name.as_deref()))
            } else {
                None
            }
        });
        match reference {
            Reference::Number(number) => captures
                .map(|(index, _)| index)
                .skip(number.saturating_sub(1))
                .take(1)
                .collect(),
            Reference::Name(wanted) => captures
                .filter(|(_, name)| *name == Some(wanted.as_str()))
                .map(|(index, _)| index)
                .collect(),
        }
    }
}

const fn capture_name(node: &RegexNode) -> Option<&str> {
    if let Kind::Capture(Some(name)) = &node.kind {
        Some(name.as_str())
    } else {
        None
    }
}

pub fn check_references(tree: &RegexTree) -> Result<(), &'static str> {
    let names: HashSet<&str> = tree.nodes.iter().filter_map(capture_name).collect();
    let dangling = tree.nodes.iter().any(|node| {
        matches!(&node.kind, Kind::Backreference(Reference::Name(name)) if !names.contains(name.as_str()))
    });
    if dangling {
        Err("Invalid named capture referenced")
    } else {
        Ok(())
    }
}

// Un mismo nombre sólo puede repetirse en alternativas distintas de una disyunción.
pub fn check_duplicate_names(tree: &RegexTree) -> Result<(), &'static str> {
    let named: Vec<(usize, &str)> = tree
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| capture_name(node).map(|name| (index, name)))
        .collect();
    let clashes = named.iter().enumerate().any(|(position, (first, name))| {
        named
            .iter()
            .skip(position.saturating_add(1))
            .any(|(second, other)| name == other && !in_different_alternatives(tree, *first, *second))
    });
    if clashes {
        Err("Duplicate capture group name")
    } else {
        Ok(())
    }
}

// Los dos caminos se separan en dos alternativas del mismo grupo.
fn in_different_alternatives(tree: &RegexTree, first: usize, second: usize) -> bool {
    let first_path = tree.path(first);
    let second_path = tree.path(second);
    let shared = first_path
        .iter()
        .rev()
        .zip(second_path.iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let is_alternative = |path: &[usize]| {
        path.len()
            .checked_sub(shared.saturating_add(1))
            .and_then(|index| path.get(index))
            .and_then(|node| tree.nodes.get(*node))
            .is_some_and(|node| node.kind == Kind::Alternative)
    };
    is_alternative(&first_path) && is_alternative(&second_path)
}
