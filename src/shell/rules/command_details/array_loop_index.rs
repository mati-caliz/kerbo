use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::Findings;
use crate::shell::parameters::{modifier, reference};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ASSIGNMENT, EXPANSION, EXPANSIONS, FOR, INDEX_FIELD, NAME_FIELD, STRING, SUBSCRIPT,
    VALUE_FIELD, VARIABLE_FIELD, WORD, ancestors, descendants, field_children, is_arithmetic_command,
    named_children, text,
};
use crate::shell::variables::assigned_name;
use crate::shell::words::braced_content;

const ALL_ELEMENTS: &str = "[@]";
const INDIRECT: char = '!';

// SC2302 y SC2303: `checkArrayValueUsedAsIndex`.
pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if node.kind() != SUBSCRIPT {
        return;
    }
    let (Some(array), Some(index)) = (
        node.child_by_field_name(NAME_FIELD),
        node.child_by_field_name(INDEX_FIELD),
    ) else {
        return;
    };
    let Some((name, reported)) = index_variable(node, index, source) else {
        return;
    };
    let array_name = text(array, source);
    for ancestor in ancestors(node).filter(|ancestor| ancestor.kind() == FOR) {
        let is_loop_variable = ancestor
            .child_by_field_name(VARIABLE_FIELD)
            .is_some_and(|variable| text(variable, source) == name);
        if !is_loop_variable {
            continue;
        }
        if is_reassigned_before(ancestor, node, &name, source) {
            return;
        }
        let looped_word = field_children(ancestor, VALUE_FIELD)
            .into_iter()
            .find(|value| looped_array(*value, source).as_deref() == Some(array_name));
        if let Some(word) = looped_word {
            findings.report(ShellRule::Sc2302, word, array_name);
            findings.report(ShellRule::Sc2303, reported, &name);
        }
        return;
    }
}

fn is_reassigned_before(loop_node: Node<'_>, subscript: Node<'_>, name: &str, source: &[u8]) -> bool {
    descendants(loop_node).into_iter().any(|node| {
        node.kind() == ASSIGNMENT
            && node.start_byte() < subscript.start_byte()
            && assigned_name(node, source).as_deref() == Some(name)
    })
}

// El nombre usado como índice y el nodo que ShellCheck marca en cada forma: el `$i` de `${a[$i]}`, y el
// `${a[i]}` o el `a[i]` aritmético enteros.
fn index_variable<'tree>(
    subscript: Node<'tree>,
    index: Node<'tree>,
    source: &[u8],
) -> Option<(String, Node<'tree>)> {
    let holder = subscript.parent()?;
    if EXPANSIONS.contains(&index.kind()) {
        let braced = braced_content(index, source);
        let name = reference(&braced);
        return (holder.kind() == EXPANSION && name == braced).then_some((name, index));
    }
    if index.kind() != WORD && index.kind() != crate::shell::syntax::VARIABLE_NAME {
        return None;
    }
    let name = text(index, source).to_owned();
    if holder.kind() == EXPANSION {
        return Some((name, holder));
    }
    let in_arithmetic = ancestors(subscript)
        .any(|ancestor| ancestor.kind() == ARITHMETIC_EXPANSION || is_arithmetic_command(ancestor, source));
    in_arithmetic.then_some((name, subscript))
}

// `getArrayName`: `${a[@]}`, también entre comillas.
fn looped_array(value: Node<'_>, source: &[u8]) -> Option<String> {
    let expansion = if value.kind() == STRING {
        match named_children(value)[..] {
            [only] => only,
            _ => return None,
        }
    } else {
        value
    };
    if !EXPANSIONS.contains(&expansion.kind()) {
        return None;
    }
    let braced = braced_content(expansion, source);
    (modifier(&braced) == ALL_ELEMENTS && !braced.starts_with(INDIRECT)).then(|| reference(&braced))
}
