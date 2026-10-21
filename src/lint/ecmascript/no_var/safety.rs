use tree_sitter::Node;

use super::{AMBIENT, Declaration, LOOP_HEAD};
use crate::lint::ecmascript::nodes::{
    STATEMENT_BLOCK, SWITCH_CASE, SWITCH_DEFAULT, is_function, without_parentheses,
};
use crate::lint::ecmascript::scope::{ScopeId, ScopeKind, ScopeTree, Variable};
use crate::lint::ecmascript::unused::is_inside;
use crate::syntax::named_children;

const DISALLOWED_NAME: &str = "let";
const SCOPE_NODES: &[&str] = &["program", "switch_statement", "for_statement", LOOP_HEAD];
const NOT_BLOCK_STATEMENTS: &[&str] = &["class_static_block", "internal_module", "module", AMBIENT];
const STATEMENT_LISTS: &[&str] = &["program", STATEMENT_BLOCK];
const LOOPS: &[&str] = &["for_statement", LOOP_HEAD, "while_statement", "do_statement"];
const HOISTED_FUNCTIONS: &[&str] = &["function_declaration", "generator_function_declaration"];

pub fn can_fix(tree: &ScopeTree<'_>, declaration: &Declaration<'_>, source: &[u8]) -> bool {
    let node = declaration.node;
    let is_loop_head = node.kind() == LOOP_HEAD;
    let parent = statement_parent(node);
    if parent.is_some_and(|parent| [SWITCH_CASE, SWITCH_DEFAULT].contains(&parent.kind())) {
        return false;
    }
    let variables: Vec<&Variable<'_>> = tree
        .declared_by(node)
        .iter()
        .filter_map(|id| tree.variable(*id))
        .collect();
    let scope_node = scope_node(node);
    let declaration_scope = tree.scope_of(node);
    let unsafe_variable = variables.iter().any(|variable| {
        variable.definitions.len() > 1
            || variable.name == DISALLOWED_NAME
            || scope_node.is_some_and(|scope_node| is_used_outside(tree, variable, scope_node))
            || is_referenced_before_declaration(tree, variable)
            || has_unsafe_hoisted_reference(tree, variable, source)
            || !resolves_to(tree, declaration_scope, variable)
    });
    if unsafe_variable || has_self_reference_in_dead_zone(tree, node) {
        return false;
    }
    if is_in_loop(node) {
        if variables
            .iter()
            .any(|variable| is_referenced_in_closure(tree, variable))
        {
            return false;
        }
        if !is_loop_head && !is_initialized(node) {
            return false;
        }
    }
    is_loop_head
        || is_for_initializer(node)
        || parent.is_some_and(|parent| STATEMENT_LISTS.contains(&parent.kind()))
}

fn statement_parent(node: Node<'_>) -> Option<Node<'_>> {
    let parent = node.parent()?;
    if parent.kind() == AMBIENT {
        parent.parent()
    } else {
        Some(parent)
    }
}

fn resolves_to(tree: &ScopeTree<'_>, scope: ScopeId, variable: &Variable<'_>) -> bool {
    tree.find(scope, &variable.name)
        .and_then(|id| tree.variable(id))
        .is_some_and(|found| std::ptr::eq(found, variable))
}

fn scope_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        let kind = candidate.kind();
        if SCOPE_NODES.contains(&kind) || (kind == STATEMENT_BLOCK && is_block_statement(candidate)) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn is_block_statement(block: Node<'_>) -> bool {
    block
        .parent()
        .is_none_or(|parent| !NOT_BLOCK_STATEMENTS.contains(&parent.kind()))
}

fn is_used_outside(tree: &ScopeTree<'_>, variable: &Variable<'_>, scope_node: Node<'_>) -> bool {
    tree.references_of(variable)
        .any(|reference| !is_inside(reference.identifier, scope_node))
}

fn declaration_start(variable: &Variable<'_>) -> Option<usize> {
    variable
        .definitions
        .first()
        .map(|definition| definition.node.start_byte())
}

fn is_referenced_before_declaration(tree: &ScopeTree<'_>, variable: &Variable<'_>) -> bool {
    let Some(start) = declaration_start(variable) else {
        return false;
    };
    tree.references_of(variable)
        .any(|reference| !reference.init && reference.identifier.start_byte() < start)
}

fn has_unsafe_hoisted_reference(tree: &ScopeTree<'_>, variable: &Variable<'_>, source: &[u8]) -> bool {
    let Some(start) = declaration_start(variable) else {
        return false;
    };
    tree.references_of(variable)
        .filter(|reference| !reference.init && reference.identifier.start_byte() >= start)
        .any(|reference| {
            let mut current = tree.variable_scope_of(reference.from);
            while current != variable.scope {
                let Some(scope) = tree.scope(current) else {
                    return false;
                };
                if is_called_before(tree, (scope.block, scope.upper), start, source) {
                    return true;
                }
                let Some(upper) = scope.upper else {
                    return false;
                };
                current = tree.variable_scope_of(upper);
            }
            false
        })
}

fn is_called_before(
    tree: &ScopeTree<'_>,
    function: (Node<'_>, Option<ScopeId>),
    start: usize,
    source: &[u8],
) -> bool {
    let (block, upper) = function;
    if !HOISTED_FUNCTIONS.contains(&block.kind()) {
        return false;
    }
    let Some(name) = block.child_by_field_name("name") else {
        return false;
    };
    let name = name.utf8_text(source).unwrap_or_default();
    let declared = upper
        .and_then(|upper| tree.scope(upper))
        .and_then(|scope| scope.lookup(name))
        .and_then(|id| tree.variable(id));
    declared.is_some_and(|called| {
        tree.references_of(called).any(|reference| {
            let identifier = reference.identifier;
            identifier.start_byte() < start
                && identifier.parent().is_some_and(|parent| {
                    parent.kind() == "call_expression"
                        && parent
                            .child_by_field_name("function")
                            .is_some_and(|callee| callee.id() == identifier.id())
                })
        })
    })
}

fn has_self_reference_in_dead_zone(tree: &ScopeTree<'_>, node: Node<'_>) -> bool {
    if node.kind() == LOOP_HEAD {
        return false;
    }
    named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "variable_declarator")
        .any(|declarator| {
            let Some(init) = declarator.child_by_field_name("value") else {
                return false;
            };
            tree.declared_by(declarator)
                .iter()
                .filter_map(|id| tree.variable(*id))
                .any(|variable| references_in_dead_zone(tree, variable, without_parentheses(init)))
        })
}

fn references_in_dead_zone(tree: &ScopeTree<'_>, variable: &Variable<'_>, init: Node<'_>) -> bool {
    let Some(name) = variable.definitions.first().map(|definition| definition.name) else {
        return false;
    };
    let default = name
        .parent()
        .filter(|parent| {
            parent.kind() == "assignment_pattern" || parent.kind() == "object_assignment_pattern"
        })
        .and_then(|parent| parent.child_by_field_name("right"));
    let init_is_function = is_function(init);
    tree.references_of(variable).any(|reference| {
        let identifier = reference.identifier;
        !reference.init
            && (identifier.start_byte() < name.start_byte()
                || default.is_some_and(|default| is_inside(identifier, default))
                || (!init_is_function && is_inside(identifier, init)))
    })
}

fn is_in_loop(node: Node<'_>) -> bool {
    let mut current = Some(node);
    while let Some(candidate) = current.filter(|candidate| !is_function(*candidate)) {
        if LOOPS.contains(&candidate.kind()) {
            return true;
        }
        current = candidate.parent();
    }
    false
}

fn enclosing_function_scope(tree: &ScopeTree<'_>, scope: ScopeId) -> ScopeId {
    let mut current = scope;
    while let Some(found) = tree.scope(current) {
        if matches!(found.kind, ScopeKind::Function | ScopeKind::Global) {
            return current;
        }
        match found.upper {
            Some(upper) => current = upper,
            None => return current,
        }
    }
    current
}

fn is_referenced_in_closure(tree: &ScopeTree<'_>, variable: &Variable<'_>) -> bool {
    let own = enclosing_function_scope(tree, variable.scope);
    tree.references_of(variable)
        .any(|reference| enclosing_function_scope(tree, reference.from) != own)
}

fn is_initialized(node: Node<'_>) -> bool {
    named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "variable_declarator")
        .all(|declarator| declarator.child_by_field_name("value").is_some())
}

fn is_for_initializer(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == "for_statement"
            && parent
                .child_by_field_name("initializer")
                .is_some_and(|initializer| initializer.id() == node.id())
    })
}
