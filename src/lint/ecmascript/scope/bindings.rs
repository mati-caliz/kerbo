use tree_sitter::Node;

use super::patterns::PatternVisitor;
use super::referencer::Referencer;
use super::types::TypeVisitor;
use super::{Access, DefinitionKind, ScopeId, Target};
use crate::lint::ecmascript::nodes::IDENTIFIER;
use crate::syntax::named_children;

const TYPE_KEYWORD: &str = "type";
const PROPERTY_MARKERS: &[&str] = &["accessibility_modifier", "override_modifier", "readonly"];

#[derive(Debug, Clone, Copy)]
pub struct ParameterStyle {
    pub used: bool,
    pub decorated_outside: bool,
}

impl ParameterStyle {
    pub const USED: Self = Self {
        used: true,
        decorated_outside: false,
    };
}

pub fn declare_declarator<'tree>(
    referencer: &mut Referencer<'tree, '_, '_>,
    declaration: Node<'tree>,
    declarator: Node<'tree>,
    target: ScopeId,
) {
    let value = declarator.child_by_field_name("value");
    if let Some(name) = declarator.child_by_field_name("name") {
        let site = (declarator, Some(declaration));
        referencer.declare_pattern(name, |inner, identifier, defaults| {
            inner.define(target, identifier, (DefinitionKind::Variable, site.0, site.1));
            inner.write_defaults(identifier, defaults, true);
            if value.is_some() {
                inner.reference(identifier, (Access::Write, Target::Value, true));
            }
        });
    }
    if let Some(value) = value {
        referencer.visit(value);
    }
    if let Some(annotation) = declarator.child_by_field_name("type") {
        referencer.visit_type(annotation);
    }
}

pub fn declare_parameter<'tree>(
    referencer: &mut Referencer<'tree, '_, '_>,
    function: Node<'tree>,
    parameter: Node<'tree>,
    style: ParameterStyle,
) {
    let scope = referencer.current;
    let used = style.used || is_parameter_property(parameter);
    referencer.declare_pattern(parameter, |inner, identifier, defaults| {
        let variable = inner.define(scope, identifier, (DefinitionKind::Parameter, function, None));
        inner.write_defaults(identifier, defaults, true);
        if used {
            inner.mark_used(variable);
        }
    });
    if let Some(annotation) = parameter.child_by_field_name("type") {
        referencer.visit_type(annotation);
    }
    if style.decorated_outside {
        return;
    }
    for decorator in named_children(parameter)
        .into_iter()
        .filter(|child| child.kind() == "decorator")
    {
        referencer.visit(decorator);
    }
}

pub fn import_specifiers<'tree>(
    referencer: &mut Referencer<'tree, '_, '_>,
    named: Node<'tree>,
    statement: Node<'tree>,
    type_only: bool,
) {
    let scope = referencer.current;
    for specifier in named_children(named)
        .into_iter()
        .filter(|child| child.kind() == "import_specifier")
    {
        let local = specifier
            .child_by_field_name("alias")
            .or_else(|| specifier.child_by_field_name("name"))
            .filter(|local| local.kind() == IDENTIFIER);
        let kind = DefinitionKind::ImportBinding {
            type_only: type_only || has_token(specifier, TYPE_KEYWORD),
        };
        if let Some(local) = local {
            referencer.define(scope, local, (kind, specifier, Some(statement)));
        }
    }
}

fn is_parameter_property(parameter: Node<'_>) -> bool {
    let mut cursor = parameter.walk();
    parameter
        .children(&mut cursor)
        .any(|child| PROPERTY_MARKERS.contains(&child.kind()))
}

pub fn has_token(node: Node<'_>, token: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| !child.is_named() && child.kind() == token)
}
