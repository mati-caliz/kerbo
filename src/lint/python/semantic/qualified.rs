use tree_sitter::Node;

use super::model::{BindingKind, Model, Resolution, ScopeId};
use crate::lint::python::ast::{ATTRIBUTE, ATTRIBUTE_FIELD, IDENTIFIER, OBJECT_FIELD, text, unparenthesized};

const BUILTINS_MODULE: &str = "builtins";
const TYPING_MODULES: [&str; 2] = ["typing", "typing_extensions"];

pub fn qualified_name(model: &Model<'_>, node: Node<'_>, source: &[u8]) -> Option<String> {
    qualified_name_from(model, model.scope, node, source)
}

pub fn qualified_name_from(
    model: &Model<'_>,
    scope: ScopeId,
    node: Node<'_>,
    source: &[u8],
) -> Option<String> {
    let mut members = Vec::new();
    let mut current = unparenthesized(node);
    while current.kind() == ATTRIBUTE {
        members.push(text(current.child_by_field_name(ATTRIBUTE_FIELD)?, source));
        current = unparenthesized(current.child_by_field_name(OBJECT_FIELD)?);
    }
    if current.kind() != IDENTIFIER {
        return None;
    }
    let head = text(current, source);
    let base = match model.lookup_from(scope, head) {
        Resolution::Builtin => format!("{BUILTINS_MODULE}.{head}"),
        Resolution::Binding(id) => {
            let binding = model.binding(id)?;
            if binding.kind == BindingKind::SubmoduleImport {
                head.to_owned()
            } else if binding.kind.is_import() {
                binding.qualified.clone()
            } else {
                return None;
            }
        }
        Resolution::Implicit | Resolution::Unresolved => return None,
    };
    Some(
        std::iter::once(base.as_str())
            .chain(members.into_iter().rev())
            .collect::<Vec<_>>()
            .join("."),
    )
}

pub fn is_typing(name: &str, member: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(module, last)| last == member && TYPING_MODULES.contains(&module))
}

pub fn is_builtin_named(model: &Model<'_>, node: Node<'_>, source: &[u8], builtin: &str) -> bool {
    qualified_name(model, node, source).is_some_and(|name| {
        name.strip_prefix(BUILTINS_MODULE)
            .and_then(|rest| rest.strip_prefix('.'))
            == Some(builtin)
    })
}
