use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use super::dependencies::{FIRST, LATEST};
use super::operations::{Migration, MigrationKey};

pub type Nodes = BTreeMap<MigrationKey, Arc<Migration>>;
pub type Parents = BTreeMap<MigrationKey, BTreeSet<MigrationKey>>;

pub struct Graph<'nodes> {
    nodes: &'nodes Nodes,
    parents: BTreeMap<&'nodes MigrationKey, BTreeSet<&'nodes MigrationKey>>,
}

impl<'nodes> Graph<'nodes> {
    pub fn new(nodes: &'nodes Nodes) -> Self {
        let mut graph = Self {
            nodes,
            parents: nodes.keys().map(|key| (key, BTreeSet::new())).collect(),
        };
        for (key, migration) in nodes {
            for dependency in migration
                .dependencies
                .iter()
                .filter(|dependency| is_internal(key, dependency))
            {
                graph.link(key, dependency);
            }
        }
        let endpoints = Endpoints {
            roots: graph.edges_by_app(true),
            leaves: graph.edges_by_app(false),
        };
        for (key, migration) in nodes {
            graph.link_external(key, migration, &endpoints);
        }
        graph
    }

    fn link_external(
        &mut self,
        key: &'nodes MigrationKey,
        migration: &Migration,
        endpoints: &Endpoints<'nodes>,
    ) {
        let current_app = key.app.as_str();
        let parents: Vec<&'nodes MigrationKey> = migration
            .dependencies
            .iter()
            .filter(|dependency| !is_internal(key, dependency))
            .filter_map(|dependency| endpoints.resolved(self.nodes, dependency, current_app))
            .collect();
        for parent in parents {
            self.link(key, parent);
        }
        let children: Vec<&'nodes MigrationKey> = migration
            .run_before
            .iter()
            .filter_map(|target| endpoints.resolved(self.nodes, target, current_app))
            .collect();
        for child in children {
            self.link(child, key);
        }
    }

    fn link(&mut self, child: &'nodes MigrationKey, parent: &MigrationKey) {
        let Some((parent, _)) = self.nodes.get_key_value(parent) else {
            return;
        };
        if let Some(parents) = self.parents.get_mut(child) {
            parents.insert(parent);
        }
    }

    fn edges_by_app(&self, roots: bool) -> BTreeMap<&'nodes str, &'nodes MigrationKey> {
        let linked: HashSet<&MigrationKey> = self
            .parents
            .iter()
            .flat_map(|(child, parents)| {
                parents
                    .iter()
                    .filter(|parent| parent.app == child.app)
                    .map(move |parent| if roots { *child } else { *parent })
            })
            .collect();
        let mut found: BTreeMap<&'nodes str, &'nodes MigrationKey> = BTreeMap::new();
        for key in self.nodes.keys().filter(|key| !linked.contains(key)) {
            found.entry(key.app.as_str()).or_insert(key);
        }
        found
    }

    pub fn into_parents(self) -> Parents {
        self.parents
            .into_iter()
            .map(|(child, parents)| (child.clone(), parents.into_iter().cloned().collect()))
            .collect()
    }
}

pub fn ancestors<'parents>(parents: &'parents Parents, start: &MigrationKey) -> Vec<&'parents MigrationKey> {
    let Some((start, _)) = parents.get_key_value(start) else {
        return Vec::new();
    };
    let mut visited = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![(start, false)];
    while let Some((node, processed)) = stack.pop() {
        if seen.contains(node) {
            continue;
        }
        if processed {
            seen.insert(node);
            visited.push(node);
            continue;
        }
        stack.push((node, true));
        if let Some(node_parents) = parents.get(node) {
            stack.extend(node_parents.iter().map(|parent| (parent, false)));
        }
    }
    visited.retain(|node| *node != start);
    visited
}

fn is_internal(key: &MigrationKey, dependency: &MigrationKey) -> bool {
    dependency.app == key.app && dependency.name != FIRST
}

struct Endpoints<'nodes> {
    roots: BTreeMap<&'nodes str, &'nodes MigrationKey>,
    leaves: BTreeMap<&'nodes str, &'nodes MigrationKey>,
}

impl<'nodes> Endpoints<'nodes> {
    fn resolved(
        &self,
        nodes: &'nodes Nodes,
        dependency: &MigrationKey,
        current_app: &str,
    ) -> Option<&'nodes MigrationKey> {
        if let Some((key, _)) = nodes.get_key_value(dependency) {
            return Some(key);
        }
        if dependency.app == current_app {
            return None;
        }
        let edges = match dependency.name.as_str() {
            FIRST => &self.roots,
            LATEST => &self.leaves,
            _ => return None,
        };
        edges.get(dependency.app.as_str()).copied()
    }
}
