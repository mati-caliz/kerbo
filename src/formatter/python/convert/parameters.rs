use tree_sitter::Node;

use super::expressions::expression;
use super::{Converter, Outcome, field, named, only_named, span, types};
use crate::formatter::python::ast::{Data, NodeId, Other, Span};

const IDENTIFIER: &str = "identifier";
const LIST_SPLAT_PATTERN: &str = "list_splat_pattern";
const DICTIONARY_SPLAT_PATTERN: &str = "dictionary_splat_pattern";

#[derive(Debug, Default)]
struct Collected {
    posonlyargs: Vec<NodeId>,
    args: Vec<NodeId>,
    vararg: Option<NodeId>,
    kwonlyargs: Vec<NodeId>,
    kwarg: Option<NodeId>,
    after_star: bool,
}

impl Collected {
    fn push(&mut self, parameter: NodeId) {
        if self.after_star {
            self.kwonlyargs.push(parameter);
        } else {
            self.args.push(parameter);
        }
    }
}

fn parameter<'tree>(
    converter: &mut Converter<'_>,
    name: Node<'tree>,
    annotation: Option<Node<'tree>>,
) -> Outcome<'tree> {
    parameter_from(converter, name.start_byte(), name, annotation)
}

// El parámetro sin su valor por defecto: el nombre y, si la tiene, la anotación. El de `*args` y
// `**kwargs` empieza en los asteriscos, como en ruff, aunque el nombre no los incluya.
fn parameter_from<'tree>(
    converter: &mut Converter<'_>,
    start: usize,
    name: Node<'tree>,
    annotation: Option<Node<'tree>>,
) -> Outcome<'tree> {
    if name.kind() != IDENTIFIER {
        return Err(name);
    }
    let end = annotation.map_or_else(|| name.end_byte(), |annotation| annotation.end_byte());
    let annotation = annotation
        .map(|annotation| types::annotation(converter, annotation))
        .transpose()?;
    let parameter = Other::Parameter {
        name: span(name),
        annotation,
    };
    Ok(converter.ast.add(Span::new(start, end), Data::Other(parameter)))
}

fn with_default<'tree>(
    converter: &mut Converter<'_>,
    node: Node<'tree>,
    parameter: NodeId,
) -> Outcome<'tree> {
    let default = node
        .child_by_field_name("value")
        .map(|value| expression(converter, value))
        .transpose()?;
    Ok(converter.other(node, Other::ParameterWithDefault { parameter, default }))
}

fn variadic<'tree>(
    converter: &mut Converter<'_>,
    splat: Node<'tree>,
    annotation: Option<Node<'tree>>,
    collected: &mut Collected,
) -> Outcome<'tree, ()> {
    let added = parameter_from(converter, splat.start_byte(), only_named(splat)?, annotation)?;
    if splat.kind() == LIST_SPLAT_PATTERN {
        collected.vararg = Some(added);
        collected.after_star = true;
    } else {
        collected.kwarg = Some(added);
    }
    Ok(())
}

fn typed<'tree>(
    converter: &mut Converter<'_>,
    node: Node<'tree>,
    collected: &mut Collected,
) -> Outcome<'tree, ()> {
    let annotation = field(node, "type")?;
    let name = named(node)
        .into_iter()
        .find(|child| *child != annotation)
        .ok_or(node)?;
    if matches!(name.kind(), LIST_SPLAT_PATTERN | DICTIONARY_SPLAT_PATTERN) {
        return variadic(converter, name, Some(annotation), collected);
    }
    let added = parameter(converter, name, Some(annotation))?;
    collected.push(with_default(converter, node, added)?);
    Ok(())
}

fn collect<'tree>(
    converter: &mut Converter<'_>,
    node: Node<'tree>,
    collected: &mut Collected,
) -> Outcome<'tree, ()> {
    match node.kind() {
        IDENTIFIER => {
            let added = parameter(converter, node, None)?;
            collected.push(with_default(converter, node, added)?);
        }
        "default_parameter" => {
            let added = parameter(converter, field(node, "name")?, None)?;
            collected.push(with_default(converter, node, added)?);
        }
        "typed_default_parameter" => {
            let added = parameter(converter, field(node, "name")?, node.child_by_field_name("type"))?;
            collected.push(with_default(converter, node, added)?);
        }
        "typed_parameter" => typed(converter, node, collected)?,
        LIST_SPLAT_PATTERN | DICTIONARY_SPLAT_PATTERN => variadic(converter, node, None, collected)?,
        "keyword_separator" => collected.after_star = true,
        "positional_separator" => collected.posonlyargs = std::mem::take(&mut collected.args),
        _ => return Err(node),
    }
    Ok(())
}

// `parameters` de un `def` (con sus paréntesis) o `lambda_parameters`.
pub fn convert<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let mut collected = Collected::default();
    for child in named(node) {
        collect(converter, child, &mut collected)?;
    }
    let parameters = Other::Parameters {
        posonlyargs: collected.posonlyargs,
        args: collected.args,
        vararg: collected.vararg,
        kwonlyargs: collected.kwonlyargs,
        kwarg: collected.kwarg,
    };
    Ok(converter.other(node, parameters))
}
