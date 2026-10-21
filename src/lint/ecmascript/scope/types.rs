use tree_sitter::Node;

use super::declarations::DeclarationVisitor;

use super::patterns::pattern_parts;
use super::referencer::Referencer;
use super::{Access, DefinitionKind, ScopeId, ScopeKind, ScopeTree, Target};
use crate::lint::ecmascript::nodes::{COMMENT, first_named_child};
use crate::syntax::named_children;

const NOTHING: &[&str] = &[
    COMMENT,
    "predefined_type",
    "this_type",
    "literal_type",
    "string",
    "number",
    "property_identifier",
    "private_property_identifier",
    "accessibility_modifier",
    "override_modifier",
    "existential_type",
];
const SIGNATURES: &[&str] = &[
    "function_type",
    "constructor_type",
    "call_signature",
    "construct_signature",
];
const TUPLE_MEMBERS: &[&str] = &["required_parameter", "optional_parameter"];
const RETURN_FIELDS: &[&str] = &["return_type", "type"];
const SKIPPED_BY_INFER: &[ScopeKind] = &[ScopeKind::FunctionType, ScopeKind::MappedType];

pub trait TypeVisitor<'tree> {
    fn visit_type(&mut self, node: Node<'tree>);
    fn visit_type_construct(&mut self, node: Node<'tree>) -> bool;
    fn visit_type_field(&mut self, node: Node<'tree>, field: &str);
    fn visit_type_declaration(&mut self, node: Node<'tree>) -> bool;
    fn visit_type_query(&mut self, node: Node<'tree>);
    fn visit_asserts(&mut self, node: Node<'tree>);
    fn signature_scope(&mut self, node: Node<'tree>);
    fn visit_index_signature(&mut self, node: Node<'tree>);
    fn visit_conditional(&mut self, node: Node<'tree>);
    fn visit_infer(&mut self, node: Node<'tree>);
    fn visit_type_parameter(&mut self, node: Node<'tree>);
}

impl<'tree> TypeVisitor<'tree> for Referencer<'tree, '_, '_> {
    fn visit_type(&mut self, node: Node<'tree>) {
        let kind = node.kind();
        if NOTHING.contains(&kind) || !node.is_named() {
            return;
        }
        if ["type_identifier", "identifier"].contains(&kind) {
            self.reference(node, (Access::Read, Target::Type, false));
        } else if SIGNATURES.contains(&kind) {
            self.signature_scope(node);
        } else if TUPLE_MEMBERS.contains(&kind) {
            if let Some(annotation) = node.child_by_field_name("type") {
                self.visit_type(annotation);
            }
        } else if !self.visit_type_declaration(node) && !self.visit_type_construct(node) {
            for child in named_children(node) {
                self.visit_type(child);
            }
        }
    }

    fn visit_type_construct(&mut self, node: Node<'tree>) -> bool {
        match node.kind() {
            "nested_type_identifier" => self.visit_type_field(node, "module"),
            "nested_identifier" | "member_expression" => self.visit_type_field(node, "object"),
            "call_expression" => self.visit_type_field(node, "type_arguments"),
            "type_query" => self.visit_type_query(node),
            "type_predicate" => {
                if let Some(name) = node
                    .child_by_field_name("name")
                    .filter(|name| name.kind() != "this")
                {
                    self.read(name);
                }
                self.visit_type_field(node, "type");
            }
            "asserts" => self.visit_asserts(node),
            "method_signature" => {
                self.visit_computed_name(node);
                self.signature_scope(node);
            }
            "property_signature" => {
                self.visit_computed_name(node);
                self.visit_type_field(node, "type");
            }
            "index_signature" => self.visit_index_signature(node),
            "conditional_type" => self.visit_conditional(node),
            "infer_type" => self.visit_infer(node),
            "type_parameter" => self.visit_type_parameter(node),
            "computed_property_name" | "export_statement" => self.visit_children(node),
            _ => return false,
        }
        true
    }

    fn visit_type_field(&mut self, node: Node<'tree>, field: &str) {
        if let Some(child) = node.child_by_field_name(field) {
            self.visit_type(child);
        }
    }

    fn visit_type_declaration(&mut self, node: Node<'tree>) -> bool {
        if !["interface_declaration", "type_alias_declaration"].contains(&node.kind()) {
            return false;
        }
        let scope = self.current;
        if let Some(name) = node.child_by_field_name("name") {
            self.define(scope, name, (DefinitionKind::Type, node, None));
        }
        let parameters = node.child_by_field_name("type_parameters");
        if let Some(parameters) = parameters {
            self.open(ScopeKind::Type, node);
            self.visit_type(parameters);
        }
        let name = node.child_by_field_name("name").map(|name| name.id());
        for child in named_children(node) {
            if Some(child.id()) != name && child.kind() != "type_parameters" {
                self.visit_type(child);
            }
        }
        if parameters.is_some() {
            self.close();
        }
        true
    }

    fn visit_type_query(&mut self, node: Node<'tree>) {
        let Some(mut expression) = first_named_child(node) else {
            return;
        };
        if expression.kind() == "instantiation_expression" {
            self.visit_type_field(expression, "type_arguments");
            match expression
                .child_by_field_name("function")
                .or_else(|| first_named_child(expression))
            {
                Some(inner) => expression = inner,
                None => return,
            }
        }
        if expression.kind() == "call_expression" {
            self.visit_type_field(expression, "type_arguments");
            return;
        }
        while let Some(object) = expression.child_by_field_name("object") {
            expression = object;
        }
        if expression.kind() == "identifier" {
            self.read(expression);
        }
    }

    fn visit_asserts(&mut self, node: Node<'tree>) {
        for child in named_children(node) {
            if child.kind() == "identifier" {
                self.read(child);
            } else {
                self.visit_type(child);
            }
        }
    }

    fn signature_scope(&mut self, node: Node<'tree>) {
        self.open(ScopeKind::FunctionType, node);
        self.visit_type_field(node, "type_parameters");
        let scope = self.current;
        let parameters = node
            .child_by_field_name("parameters")
            .map(named_children)
            .unwrap_or_default();
        for parameter in parameters.into_iter().filter(|child| child.kind() != COMMENT) {
            for identifier in pattern_parts(parameter).identifiers {
                let variable = self.define(scope, identifier.node, (DefinitionKind::Parameter, node, None));
                self.mark_used(variable);
            }
            self.visit_type_field(parameter, "type");
        }
        for field in RETURN_FIELDS {
            self.visit_type_field(node, field);
        }
        self.close();
    }

    fn visit_index_signature(&mut self, node: Node<'tree>) {
        let mapped = named_children(node)
            .into_iter()
            .find(|child| child.kind() == "mapped_type_clause");
        let Some(clause) = mapped else {
            self.visit_type_field(node, "index_type");
            self.visit_type_field(node, "type");
            return;
        };
        self.open(ScopeKind::MappedType, node);
        let scope = self.current;
        if let Some(name) = clause.child_by_field_name("name") {
            let variable = self.define(scope, name, (DefinitionKind::Type, node, None));
            self.mark_used(variable);
        }
        self.visit_type_field(clause, "type");
        self.visit_type_field(clause, "alias");
        self.visit_type_field(node, "type");
        self.close();
    }

    fn visit_conditional(&mut self, node: Node<'tree>) {
        self.open(ScopeKind::ConditionalType, node);
        for field in ["left", "right", "consequence"] {
            self.visit_type_field(node, field);
        }
        self.close();
        self.visit_type_field(node, "alternative");
    }

    fn visit_infer(&mut self, node: Node<'tree>) {
        let target = infer_scope(self.tree, self.current);
        let mut parts = named_children(node)
            .into_iter()
            .filter(|child| child.kind() != COMMENT);
        if let Some(name) = parts.next() {
            self.define(target, name, (DefinitionKind::Type, node, None));
        }
        for constraint in parts {
            self.visit_type(constraint);
        }
    }

    fn visit_type_parameter(&mut self, node: Node<'tree>) {
        let scope = self.current;
        if let Some(name) = node.child_by_field_name("name") {
            self.define(scope, name, (DefinitionKind::Type, node, None));
        }
        self.visit_type_field(node, "constraint");
        self.visit_type_field(node, "value");
    }
}

fn infer_scope(tree: &ScopeTree<'_>, current: ScopeId) -> ScopeId {
    let skipped = |scope: ScopeId| {
        tree.scope(scope)
            .is_some_and(|found| SKIPPED_BY_INFER.contains(&found.kind))
    };
    if !skipped(current) {
        return current;
    }
    let mut upper = tree.scope(current).and_then(|scope| scope.upper);
    while let Some(candidate) = upper.filter(|candidate| skipped(*candidate)) {
        upper = tree.scope(candidate).and_then(|scope| scope.upper);
    }
    upper
        .filter(|candidate| {
            tree.scope(*candidate)
                .is_some_and(|found| found.kind == ScopeKind::ConditionalType)
        })
        .unwrap_or(current)
}
