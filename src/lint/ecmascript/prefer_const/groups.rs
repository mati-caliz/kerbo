use tree_sitter::Node;

use super::{
    LOOP_HEAD, PATTERN_PARENTS, identifier_if_should_be_const, is_parameter, keyword_fix, report_identifier,
};
use crate::lint::LintFinding;
use crate::lint::ecmascript::nodes::{COMMENT, IDENTIFIER, PARENTHESIZED, text, without_parentheses};
use crate::lint::ecmascript::scope::{Reference, ScopeTree, Variable, VariableId};
use crate::syntax::named_children;

const DECLARATOR: &str = "variable_declarator";
const ASSIGNMENTS: &[&str] = &["assignment_expression", "augmented_assignment_expression"];
const OBJECT_PATTERN: &str = "object_pattern";
const ARRAY_PATTERN: &str = "array_pattern";
const MEMBER_TARGETS: &[&str] = &["member_expression", "subscript_expression"];
const SHORTHAND: &str = "shorthand_property_identifier_pattern";

#[derive(Debug, Clone, Copy)]
pub enum DestructuringHost<'tree> {
    Declarator(Node<'tree>),
    Assignment(Node<'tree>),
}

impl DestructuringHost<'_> {
    fn node_id(&self) -> usize {
        match self {
            Self::Declarator(node) | Self::Assignment(node) => node.id(),
        }
    }
}

pub fn destructuring_host<'tree>(reference: &Reference<'tree>) -> Option<DestructuringHost<'tree>> {
    if !reference.is_write() {
        return None;
    }
    let mut node = reference.identifier.parent();
    while let Some(current) =
        node.filter(|current| PATTERN_PARENTS.contains(&current.kind()) || current.kind() == PARENTHESIZED)
    {
        node = current.parent();
    }
    let node = node?;
    match node.kind() {
        DECLARATOR => Some(DestructuringHost::Declarator(node)),
        LOOP_HEAD if node.child_by_field_name("kind").is_some() => Some(DestructuringHost::Declarator(node)),
        kind if ASSIGNMENTS.contains(&kind) => Some(DestructuringHost::Assignment(node)),
        _ => None,
    }
}

pub fn group_by_destructuring<'tree>(
    tree: &ScopeTree<'tree>,
    variables: &[VariableId],
    source: &[u8],
) -> Vec<Vec<Option<Node<'tree>>>> {
    let mut groups: Vec<(DestructuringHost<'tree>, Vec<Option<Node<'tree>>>)> = Vec::new();
    for variable in variables.iter().filter_map(|id| tree.variable(*id)) {
        let identifier = identifier_if_should_be_const(tree, variable, source);
        let mut previous: Option<usize> = None;
        for reference in tree.references_of(variable) {
            let id = reference.identifier.id();
            if previous == Some(id) {
                continue;
            }
            previous = Some(id);
            let Some(host) = destructuring_host(reference) else {
                continue;
            };
            match groups
                .iter_mut()
                .find(|(existing, _)| existing.node_id() == host.node_id())
            {
                Some((_, identifiers)) => identifiers.push(identifier),
                None => groups.push((host, vec![identifier])),
            }
        }
    }
    groups.into_iter().map(|(_, identifiers)| identifiers).collect()
}

pub fn has_outer_or_member_targets(
    tree: &ScopeTree<'_>,
    assignment: Node<'_>,
    variable: &Variable<'_>,
    source: &[u8],
) -> bool {
    let Some(left) = assignment.child_by_field_name("left").map(without_parentheses) else {
        return false;
    };
    let names: Vec<Option<Node<'_>>> = match left.kind() {
        OBJECT_PATTERN => named_children(left)
            .into_iter()
            .filter_map(|property| match property.kind() {
                SHORTHAND => Some(Some(property)),
                "pair_pattern" => Some(
                    property
                        .child_by_field_name("value")
                        .filter(|value| value.kind() == IDENTIFIER),
                ),
                "object_assignment_pattern" => Some(None),
                _ => None,
            })
            .collect(),
        ARRAY_PATTERN => named_children(left)
            .into_iter()
            .filter(|element| element.kind() != COMMENT)
            .map(|element| (element.kind() == IDENTIFIER).then_some(element))
            .collect(),
        _ => return false,
    };
    let source_names: Vec<&str> = names
        .into_iter()
        .flatten()
        .map(|name| text(name, source))
        .collect();
    source_names
        .iter()
        .any(|name| is_outer_variable(tree, name, variable.scope))
        || has_member_target(left)
}

fn is_outer_variable(tree: &ScopeTree<'_>, name: &str, scope: usize) -> bool {
    let passes_through = tree.scope(scope).is_some_and(|found| {
        found.through.iter().any(|id| {
            tree.reference(*id)
                .and_then(|reference| reference.resolved)
                .and_then(|resolved| tree.variable(resolved))
                .is_some_and(|resolved| resolved.name == name)
        })
    });
    passes_through
        || tree
            .find(scope, name)
            .and_then(|id| tree.variable(id))
            .is_some_and(is_parameter)
}

fn has_member_target(node: Node<'_>) -> bool {
    let node = without_parentheses(node);
    match node.kind() {
        OBJECT_PATTERN | ARRAY_PATTERN | "rest_pattern" => {
            named_children(node).into_iter().any(has_member_target)
        }
        "pair_pattern" => node.child_by_field_name("value").is_some_and(has_member_target),
        "assignment_pattern" | "object_assignment_pattern" => {
            node.child_by_field_name("left").is_some_and(has_member_target)
        }
        kind => MEMBER_TARGETS.contains(&kind),
    }
}

#[derive(Debug)]
pub struct Reporter<'source> {
    source: &'source [u8],
    checked_name: Option<String>,
    checked_id: Option<usize>,
    report_count: usize,
    findings: Vec<LintFinding>,
}

impl<'source> Reporter<'source> {
    pub const fn new(source: &'source [u8]) -> Self {
        Self {
            source,
            checked_name: Some(String::new()),
            checked_id: None,
            report_count: 0,
            findings: Vec::new(),
        }
    }

    pub fn check(&mut self, identifiers: &[Option<Node<'_>>]) {
        let reported: Vec<Node<'_>> = identifiers.iter().flatten().copied().collect();
        let Some(first) = identifiers.first() else {
            return;
        };
        let declaration = first.and_then(enclosing_declaration);
        let declarators = declaration.map(declarators).unwrap_or_default();
        if let Some(first_declarator) = declarators.first().filter(|declarator| declarator.init.is_some()) {
            self.track(first_declarator);
        }
        let mut should_fix = declaration.is_some()
            && (declaration.is_some_and(|node| node.kind() == LOOP_HEAD)
                || declarators.iter().all(|declarator| declarator.init.is_some()))
            && reported.len() == identifiers.len();
        if declaration.is_some() && declarators.len() != 1 {
            self.report_count = self.report_count.saturating_add(reported.len());
            let total: usize = declarators.iter().map(|declarator| declarator.bindings).sum();
            should_fix = should_fix && self.report_count == total;
        }
        for identifier in reported {
            let fix = declaration
                .filter(|_| should_fix)
                .and_then(|declaration| keyword_fix(declaration, self.source));
            self.findings
                .push(report_identifier(identifier, text(identifier, self.source), fix));
        }
    }

    pub fn into_findings(self) -> Vec<LintFinding> {
        self.findings
    }

    fn track(&mut self, declarator: &Declarator<'_>) {
        let name = (declarator.id.kind() == IDENTIFIER).then(|| text(declarator.id, self.source).to_owned());
        if name != self.checked_name {
            self.checked_name = name;
            self.report_count = 0;
        }
        if declarator.id.kind() == OBJECT_PATTERN {
            let initial = declarator
                .init
                .filter(|init| init.kind() == IDENTIFIER)
                .map(|init| text(init, self.source).to_owned());
            if initial != self.checked_name {
                self.checked_name = initial;
                self.report_count = 0;
            }
        }
        if Some(declarator.id.id()) != self.checked_id {
            self.checked_id = Some(declarator.id.id());
            self.report_count = 0;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Declarator<'tree> {
    id: Node<'tree>,
    init: Option<Node<'tree>>,
    bindings: usize,
}

fn enclosing_declaration(identifier: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(identifier);
    while let Some(node) = current {
        let kind = node.kind();
        if kind == "lexical_declaration" || kind == "variable_declaration" {
            return Some(node);
        }
        if kind == LOOP_HEAD {
            return node.child_by_field_name("kind").map(|_| node);
        }
        if kind.ends_with("_statement") || kind == "statement_block" {
            return None;
        }
        current = node.parent();
    }
    None
}

fn declarators(declaration: Node<'_>) -> Vec<Declarator<'_>> {
    if declaration.kind() == LOOP_HEAD {
        return declaration
            .child_by_field_name("left")
            .map(|id| Declarator {
                id,
                init: None,
                bindings: 1,
            })
            .into_iter()
            .collect();
    }
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == DECLARATOR)
        .filter_map(|declarator| {
            let id = declarator.child_by_field_name("name")?;
            Some(Declarator {
                id,
                init: declarator.child_by_field_name("value").map(without_parentheses),
                bindings: binding_count(id),
            })
        })
        .collect()
}

fn binding_count(id: Node<'_>) -> usize {
    match id.kind() {
        OBJECT_PATTERN => named_children(id)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
            .count(),
        ARRAY_PATTERN => array_elements(id),
        _ => 1,
    }
}

fn array_elements(pattern: Node<'_>) -> usize {
    let mut cursor = pattern.walk();
    let mut count = 0_usize;
    let mut expecting_element = true;
    for child in pattern.children(&mut cursor) {
        match child.kind() {
            "[" | "]" | COMMENT => {}
            "," => {
                if expecting_element {
                    count = count.saturating_add(1);
                }
                expecting_element = true;
            }
            _ => {
                count = count.saturating_add(1);
                expecting_element = false;
            }
        }
    }
    count
}
