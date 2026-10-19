use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use crate::syntax::{descendants, named_children};

const FUNCTION: &str = "function_declaration";
const METHOD: &str = "method_declaration";
const CALL: &str = "call_expression";
const GENERIC_CONVERSION: &str = "type_conversion_expression";
const GENERIC_TYPE: &str = "generic_type";
const IDENTIFIER: &str = "identifier";
const TYPE_IDENTIFIER: &str = "type_identifier";
const SELECTOR: &str = "selector_expression";
const WRAPPERS: &[&str] = &["parenthesized_expression", "index_expression"];
const POINTER: &str = "pointer_type";

struct Declaration<'tree> {
    node: Node<'tree>,
    name: &'tree str,
    receiver_type: Option<&'tree str>,
    receiver_names: Vec<&'tree str>,
}

pub fn recursive_units(root: Node<'_>, source: &[u8]) -> HashSet<usize> {
    let declarations: Vec<Declaration<'_>> = descendants(root)
        .into_iter()
        .filter(|node| {
            [FUNCTION, METHOD].contains(&node.kind()) && node.child_by_field_name("body").is_some()
        })
        .filter_map(|node| declaration(node, source))
        .collect();
    let calls: HashMap<usize, Vec<usize>> = declarations
        .iter()
        .map(|caller| (caller.node.id(), callees(caller, &declarations, source)))
        .collect();
    declarations
        .iter()
        .map(|declaration| declaration.node.id())
        .filter(|&unit| reaches(unit, unit, &calls))
        .collect()
}

fn declaration<'tree>(node: Node<'tree>, source: &'tree [u8]) -> Option<Declaration<'tree>> {
    let name = node.child_by_field_name("name")?.utf8_text(source).ok()?;
    let receiver = node
        .child_by_field_name("receiver")
        .and_then(|list| named_children(list).into_iter().next());
    Some(Declaration {
        node,
        name,
        receiver_type: receiver
            .and_then(|parameter| parameter.child_by_field_name("type"))
            .and_then(|receiver_type| base_type(receiver_type, source)),
        receiver_names: receiver
            .map(|parameter| {
                let mut cursor = parameter.walk();
                parameter
                    .children_by_field_name("name", &mut cursor)
                    .filter_map(|receiver_name| receiver_name.utf8_text(source).ok())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn base_type<'tree>(node: Node<'_>, source: &'tree [u8]) -> Option<&'tree str> {
    match node.kind() {
        POINTER | GENERIC_TYPE => named_children(node)
            .into_iter()
            .find_map(|inner| base_type(inner, source)),
        TYPE_IDENTIFIER => node.utf8_text(source).ok(),
        _ => None,
    }
}

fn callees(caller: &Declaration<'_>, declarations: &[Declaration<'_>], source: &[u8]) -> Vec<usize> {
    descendants(caller.node)
        .into_iter()
        .filter_map(|node| match node.kind() {
            CALL => node.child_by_field_name("function").map(unwrapped),
            GENERIC_CONVERSION => node
                .child_by_field_name("type")
                .filter(|target| target.kind() == GENERIC_TYPE),
            _ => None,
        })
        .flat_map(|callee| resolve(callee, caller, declarations, source))
        .collect()
}

fn unwrapped(callee: Node<'_>) -> Node<'_> {
    let mut current = callee;
    while WRAPPERS.contains(&current.kind()) {
        let Some(inner) = current
            .child_by_field_name("operand")
            .or_else(|| named_children(current).into_iter().next())
        else {
            break;
        };
        current = inner;
    }
    current
}

fn resolve(
    callee: Node<'_>,
    current: &Declaration<'_>,
    declarations: &[Declaration<'_>],
    source: &[u8],
) -> Vec<usize> {
    let text = |node: Node<'_>| node.utf8_text(source).ok();
    let (name, receiver_type) = match callee.kind() {
        IDENTIFIER => (text(callee), None),
        GENERIC_TYPE => (callee.child_by_field_name("type").and_then(text), None),
        SELECTOR => {
            let through_receiver = callee
                .child_by_field_name("operand")
                .filter(|operand| operand.kind() == IDENTIFIER)
                .and_then(text)
                .is_some_and(|operand| current.receiver_names.contains(&operand));
            if !through_receiver || current.receiver_type.is_none() {
                return Vec::new();
            }
            (
                callee.child_by_field_name("field").and_then(text),
                current.receiver_type,
            )
        }
        _ => return Vec::new(),
    };
    let Some(name) = name else {
        return Vec::new();
    };
    declarations
        .iter()
        .filter(|target| target.name == name && target.receiver_type == receiver_type)
        .filter(|target| receiver_type.is_some() == (target.node.kind() == METHOD))
        .map(|target| target.node.id())
        .collect()
}

fn reaches(start: usize, goal: usize, calls: &HashMap<usize, Vec<usize>>) -> bool {
    let mut seen = HashSet::new();
    let mut pending: Vec<usize> = calls.get(&start).cloned().unwrap_or_default();
    while let Some(unit) = pending.pop() {
        if unit == goal {
            return true;
        }
        if seen.insert(unit) {
            pending.extend(calls.get(&unit).into_iter().flatten());
        }
    }
    false
}
