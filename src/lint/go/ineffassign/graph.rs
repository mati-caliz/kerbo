use std::collections::{HashMap, HashSet};

use crate::lint::go::ast::IdentId;
use crate::lint::go::resolver::ObjectId;

pub type BlockId = usize;
pub type BranchId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation {
    pub ident: IdentId,
    pub assign: bool,
}

#[derive(Debug, Default)]
pub struct Block {
    pub children: Vec<BlockId>,
    pub operations: HashMap<ObjectId, Vec<Operation>>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Variable {
    pub function_depth: i64,
    pub escapes: bool,
}

#[derive(Debug)]
struct Branch {
    label: Option<ObjectId>,
    sources: Vec<BlockId>,
    destination: Option<BlockId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchTarget {
    Innermost,
    Labeled(Option<ObjectId>),
}

#[derive(Debug, Default)]
pub struct Graph {
    blocks: Vec<Block>,
    branches: Vec<Branch>,
}

impl Graph {
    pub fn new_block(&mut self, parents: &[BlockId]) -> BlockId {
        self.blocks.push(Block::default());
        let block = self.blocks.len().saturating_sub(1);
        for parent in parents {
            self.add_child(*parent, block);
        }
        block
    }

    pub fn add_child(&mut self, parent: BlockId, child: BlockId) {
        if let Some(parent) = self.blocks.get_mut(parent) {
            parent.children.push(child);
        }
    }

    pub fn record(&mut self, block: BlockId, object: ObjectId, operation: Operation) {
        if let Some(block) = self.blocks.get_mut(block) {
            block.operations.entry(object).or_default().push(operation);
        }
    }

    pub fn new_branch(&mut self, label: Option<ObjectId>) -> BranchId {
        self.branches.push(Branch {
            label,
            sources: Vec::new(),
            destination: None,
        });
        self.branches.len().saturating_sub(1)
    }

    pub fn branch_label(&self, branch: BranchId) -> Option<ObjectId> {
        self.branches.get(branch).and_then(|branch| branch.label)
    }

    pub fn add_source(&mut self, branch: BranchId, source: BlockId) {
        let Some(entry) = self.branches.get_mut(branch) else {
            return;
        };
        entry.sources.push(source);
        if let Some(destination) = entry.destination {
            self.add_child(source, destination);
        }
    }

    pub fn set_destination(&mut self, branch: BranchId, destination: BlockId) {
        let Some(entry) = self.branches.get_mut(branch) else {
            return;
        };
        entry.destination = Some(destination);
        for source in entry.sources.clone() {
            self.add_child(source, destination);
        }
    }

    pub fn ineffectual(&self, roots: &[BlockId], variables: &HashMap<ObjectId, Variable>) -> Vec<IdentId> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        let mut pending: Vec<BlockId> = roots.iter().rev().copied().collect();
        while let Some(block_id) = pending.pop() {
            if !seen.insert(block_id) {
                continue;
            }
            let Some(block) = self.blocks.get(block_id) else {
                continue;
            };
            for object in block.operations.keys() {
                let escapes = variables.get(object).is_some_and(|variable| variable.escapes);
                found.extend(self.ineffectual_in(block, *object, escapes));
            }
            pending.extend(block.children.iter().rev().copied());
        }
        found
    }

    fn ineffectual_in(&self, block: &Block, object: ObjectId, escapes: bool) -> Vec<IdentId> {
        let operations = block.operations.get(&object).map_or(&[][..], Vec::as_slice);
        operations
            .iter()
            .enumerate()
            .filter(|(_, operation)| operation.assign)
            .filter(|(index, _)| {
                operations
                    .get(index.saturating_add(1))
                    .map_or_else(|| !escapes && !self.used_after(block, object), |next| next.assign)
            })
            .map(|(_, operation)| operation.ident)
            .collect()
    }

    fn used_after(&self, block: &Block, object: ObjectId) -> bool {
        let mut seen = HashSet::new();
        let mut pending: Vec<BlockId> = block.children.clone();
        while let Some(block_id) = pending.pop() {
            if !seen.insert(block_id) {
                continue;
            }
            let Some(candidate) = self.blocks.get(block_id) else {
                continue;
            };
            match candidate
                .operations
                .get(&object)
                .and_then(|operations| operations.first())
            {
                Some(first) if !first.assign => return true,
                Some(_) => {}
                None => pending.extend(candidate.children.iter().copied()),
            }
        }
        false
    }
}

#[derive(Debug, Default)]
pub struct BranchStack {
    entries: Vec<BranchId>,
}

impl BranchStack {
    pub fn push(&mut self, graph: &mut Graph, label: Option<ObjectId>) -> BranchId {
        let branch = graph.new_branch(label);
        self.entries.push(branch);
        branch
    }

    pub fn pop(&mut self) {
        self.entries.pop();
    }

    pub fn get(&mut self, graph: &mut Graph, target: BranchTarget) -> BranchId {
        let found = self
            .entries
            .iter()
            .rev()
            .find(|branch| match target {
                BranchTarget::Innermost => true,
                BranchTarget::Labeled(label) => graph.branch_label(**branch) == label,
            })
            .copied();
        match (found, target) {
            (Some(branch), _) => branch,
            (None, BranchTarget::Innermost) => graph.new_branch(None),
            (None, BranchTarget::Labeled(label)) => self.push(graph, label),
        }
    }
}
