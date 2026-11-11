use tree_sitter::Node;

use super::constants::Constants;
use super::dependencies::swappable_default;
use super::values::text;
use crate::deploy::python_calls::Call;
use crate::syntax::named_children;

const STRING: &str = "string";
const CONCATENATED_STRING: &str = "concatenated_string";
const STRING_CONTENT: &str = "string_content";
const INTERPOLATION: &str = "interpolation";
const CALL: &str = "call";
const SUBSCRIPT: &str = "subscript";
const INTEGER: &str = "integer";
const LOWER_METHOD: &str = "lower";
const SWAPPER_MODULE: &str = "swapper";
const SWAPPER_SPLIT: &str = "split";
const APP_PART: &str = "0";
const MODEL_PART: &str = "1";

pub fn name_text(node: Node<'_>, constants: Constants<'_>) -> Option<String> {
    let resolved = constants.resolve(node);
    let parts = named_children(resolved);
    if resolved.kind() == CONCATENATED_STRING {
        return parts.into_iter().map(|part| name_text(part, constants)).collect();
    }
    if resolved.kind() != STRING || !parts.iter().any(|part| part.kind() == INTERPOLATION) {
        return text(resolved, constants.source);
    }
    parts
        .into_iter()
        .filter_map(|part| match part.kind() {
            STRING_CONTENT => Some(part.utf8_text(constants.source).ok().map(str::to_owned)),
            INTERPOLATION => Some(evaluated(named_children(part).into_iter().next()?, constants)),
            _ => None,
        })
        .collect()
}

fn evaluated(node: Node<'_>, constants: Constants<'_>) -> Option<String> {
    let resolved = constants.resolve(node);
    match resolved.kind() {
        CALL => lowered(resolved, constants),
        SUBSCRIPT => swapped_part(resolved, constants),
        _ => text(resolved, constants.source),
    }
}

fn lowered(node: Node<'_>, constants: Constants<'_>) -> Option<String> {
    let function = node.child_by_field_name("function")?;
    let method = function
        .child_by_field_name("attribute")?
        .utf8_text(constants.source)
        .ok()?;
    if method != LOWER_METHOD {
        return None;
    }
    Some(evaluated(function.child_by_field_name("object")?, constants)?.to_lowercase())
}

fn swapped_part(node: Node<'_>, constants: Constants<'_>) -> Option<String> {
    let position = node.child_by_field_name("subscript")?;
    if position.kind() != INTEGER {
        return None;
    }
    let split = Call::parse(
        constants.resolve(node.child_by_field_name("value")?),
        constants.source,
    )?;
    if split.receiver != SWAPPER_MODULE || split.method != SWAPPER_SPLIT {
        return None;
    }
    let model = swappable_default(constants.resolve(split.positional(0)?), constants.source)?;
    match position.utf8_text(constants.source).ok()? {
        APP_PART => Some(model.app),
        MODEL_PART => Some(model.model),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::django::constants::assignments;
    use crate::deploy::django::tests::parsed;

    #[test]
    fn a_field_named_after_the_swapped_page_model_takes_its_lowercase_name() {
        let source = "page_model_name = swapper.split(swapper.get_model_name(\"wagtailcore\", \"Page\"))[1]\n\
                      parent_rel_name = f\"{page_model_name.lower()}_ptr\"\n\
                      parent_rel_name\n";
        let tree = parsed(source);
        let root = tree.root_node();
        let bindings = assignments(root, source.as_bytes());
        let constants = Constants::new(source.as_bytes(), &bindings);
        let usage = named_children(named_children(root)[2])[0];
        assert_eq!(name_text(usage, constants).as_deref(), Some("page_ptr"));
    }
}
