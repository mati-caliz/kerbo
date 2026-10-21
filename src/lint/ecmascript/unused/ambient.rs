use tree_sitter::Node;

use crate::lint::ecmascript::nodes::{COMMENT, IDENTIFIER, STATEMENT_BLOCK, first_named_child, text};
use crate::lint::ecmascript::scope::ScopeTree;
use crate::lint::ecmascript::scope::patterns::pattern_parts;
use crate::syntax::{descendants, named_children};

const AMBIENT: &str = "ambient_declaration";
const MODULES: &[&str] = &["internal_module", "module"];
const GLOBAL: &str = "global";
const NAMESPACE: &str = "namespace";
const EXPORT: &str = "export_statement";
const UNWRAPPED: &[&str] = &[AMBIENT, "expression_statement"];
const NAMED_DECLARATIONS: &[&str] = &[
    "interface_declaration",
    "type_alias_declaration",
    "class_declaration",
    "abstract_class_declaration",
    "function_signature",
    "enum_declaration",
    "internal_module",
    "module",
];
const VARIABLE_DECLARATIONS: &[&str] = &["variable_declaration", "lexical_declaration"];
const RETURN: &str = "return_statement";

pub fn mark_ambient_declarations(
    tree: &mut ScopeTree<'_>,
    root: Node<'_>,
    source: &[u8],
    declaration_file: bool,
) {
    if declaration_file && !has_overriding_export(root) {
        mark_children(tree, root, source);
    }
    for module in descendants(root).into_iter().filter(|node| is_module(*node)) {
        let applies = declaration_file || is_declared(module) || is_inside_declared_module(module);
        if !applies {
            continue;
        }
        if let Some(body) = module_body(module).filter(|body| !has_overriding_export(*body)) {
            mark_children(tree, body, source);
        }
    }
}

pub fn mark_loop_returns(tree: &mut ScopeTree<'_>, root: Node<'_>, source: &[u8]) {
    for statement in descendants(root)
        .into_iter()
        .filter(|node| node.kind() == "for_in_statement")
    {
        if !returns_at_once(statement) {
            continue;
        }
        let Some(left) = statement.child_by_field_name("left") else {
            continue;
        };
        let variable = if statement.child_by_field_name("kind").is_some() {
            tree.declared_by(statement).first().copied()
        } else if left.kind() == IDENTIFIER {
            tree.find(tree.scope_of(left), text(left, source))
        } else {
            None
        };
        if let Some(found) = variable.and_then(|id| tree.variables.get_mut(id)) {
            found.used = true;
        }
    }
}

fn returns_at_once(statement: Node<'_>) -> bool {
    let Some(body) = statement.child_by_field_name("body") else {
        return false;
    };
    if body.kind() != STATEMENT_BLOCK {
        return body.kind() == RETURN;
    }
    let statements: Vec<Node<'_>> = named_children(body)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    matches!(statements.as_slice(), [only] if only.kind() == RETURN)
}

fn is_module(node: Node<'_>) -> bool {
    MODULES.contains(&node.kind())
        || (node.kind() == AMBIENT && super::super::scope::bindings::has_token(node, GLOBAL))
}

fn is_declared(module: Node<'_>) -> bool {
    module.kind() == AMBIENT || module.parent().is_some_and(|parent| parent.kind() == AMBIENT)
}

fn is_inside_declared_module(module: Node<'_>) -> bool {
    let mut current = module.parent();
    while let Some(ancestor) = current {
        if is_module(ancestor) && is_declared(ancestor) {
            return true;
        }
        current = ancestor.parent();
    }
    false
}

fn module_body(module: Node<'_>) -> Option<Node<'_>> {
    module.child_by_field_name("body").or_else(|| {
        named_children(module)
            .into_iter()
            .find(|child| child.kind() == STATEMENT_BLOCK)
    })
}

fn has_overriding_export(container: Node<'_>) -> bool {
    named_children(container)
        .into_iter()
        .filter(|statement| statement.kind() == EXPORT)
        .any(|statement| {
            statement.child_by_field_name("declaration").is_none()
                && !super::super::scope::bindings::has_token(statement, NAMESPACE)
                && statement
                    .child_by_field_name("value")
                    .is_none_or(|value| value.kind() == IDENTIFIER)
        })
}

fn mark_children(tree: &mut ScopeTree<'_>, container: Node<'_>, source: &[u8]) {
    let scope = tree.variable_scope_of(tree.scope_of(container));
    let names: Vec<String> = named_children(container)
        .into_iter()
        .map(unwrapped)
        .flat_map(|declaration| declared_names(declaration, source))
        .collect();
    for name in names {
        let variable = tree.scope(scope).and_then(|found| found.lookup(&name));
        if let Some(found) = variable.and_then(|id| tree.variables.get_mut(id)) {
            found.used = true;
        }
    }
}

fn unwrapped(statement: Node<'_>) -> Node<'_> {
    let mut current = statement;
    while UNWRAPPED.contains(&current.kind()) {
        match first_named_child(current) {
            Some(inner) if !has_global(current) => current = inner,
            _ => break,
        }
    }
    current
}

fn has_global(node: Node<'_>) -> bool {
    node.kind() == AMBIENT && super::super::scope::bindings::has_token(node, GLOBAL)
}

fn declared_names(declaration: Node<'_>, source: &[u8]) -> Vec<String> {
    if NAMED_DECLARATIONS.contains(&declaration.kind()) {
        return declaration
            .child_by_field_name("name")
            .filter(|name| name.kind() != "string" && name.kind() != "nested_identifier")
            .map(|name| vec![text(name, source).to_owned()])
            .unwrap_or_default();
    }
    if VARIABLE_DECLARATIONS.contains(&declaration.kind()) {
        return named_children(declaration)
            .into_iter()
            .filter_map(|declarator| declarator.child_by_field_name("name"))
            .flat_map(|pattern| pattern_parts(pattern).identifiers)
            .map(|identifier| text(identifier.node, source).to_owned())
            .collect();
    }

    Vec::new()
}
