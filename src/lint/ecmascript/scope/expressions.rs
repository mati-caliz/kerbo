use tree_sitter::Node;

use super::declarations::DeclarationVisitor;
use super::patterns::PatternVisitor;
use super::types::TypeVisitor;

use super::referencer::Referencer;
use super::{Access, DefinitionKind, Target};
use crate::lint::ecmascript::nodes::{COMMENT, first_named_child, without_parentheses};
use crate::syntax::named_children;

const PATTERN_TARGETS: &[&str] = &["identifier", "undefined", "object_pattern", "array_pattern"];
const TYPE_WRAPPERS: &[&str] = &["as_expression", "satisfies_expression"];
const JSX_TAGS: &[&str] = &[
    "jsx_opening_element",
    "jsx_self_closing_element",
    "jsx_closing_element",
];
const JSX_FACTORY: &str = "React";
const SIMPLE_ASSIGNMENT: &str = "=";
const USING: &str = "using";

pub trait ExpressionVisitor<'tree> {
    fn visit_expression(&mut self, node: Node<'tree>) -> bool;
    fn visit_type_wrapper(&mut self, node: Node<'tree>);
    fn is_expression_child(wrapper: Node<'_>, child: Node<'_>) -> bool;
    fn expression_target(&mut self, node: Node<'tree>) -> Node<'tree>;
    fn visit_assignment(&mut self, node: Node<'tree>);
    fn visit_using(&mut self, node: Node<'tree>);
    fn visit_update(&mut self, node: Node<'tree>);
    fn visit_call(&mut self, node: Node<'tree>);
    fn visit_object(&mut self, node: Node<'tree>);
    fn visit_computed_key(&mut self, node: Node<'tree>);
    fn visit_jsx_attribute(&mut self, node: Node<'tree>);
    fn visit_jsx_tag(&mut self, node: Node<'tree>);
    fn visit_jsx_name(&mut self, name: Node<'tree>);
    fn visit_jsx_object(&mut self, object: Node<'tree>);
    fn reference_jsx_factory(&mut self);
}

impl<'tree> ExpressionVisitor<'tree> for Referencer<'tree, '_, '_> {
    fn visit_expression(&mut self, node: Node<'tree>) -> bool {
        match node.kind() {
            "assignment_expression" | "augmented_assignment_expression" => self.visit_assignment(node),
            "update_expression" => self.visit_update(node),
            "member_expression" => self.visit_field(node, "object"),
            "subscript_expression" => {
                self.visit_field(node, "object");
                self.visit_field(node, "index");
            }
            "call_expression" | "new_expression" | "instantiation_expression" => self.visit_call(node),
            "object" => self.visit_object(node),
            "pair" => {
                self.visit_computed_key(node);
                self.visit_field(node, "value");
            }
            "jsx_attribute" => self.visit_jsx_attribute(node),
            kind if JSX_TAGS.contains(&kind) => self.visit_jsx_tag(node),
            kind if TYPE_WRAPPERS.contains(&kind) => self.visit_type_wrapper(node),
            "type_assertion" => self.visit_type_wrapper(node),
            _ => return false,
        }
        true
    }

    fn visit_type_wrapper(&mut self, node: Node<'tree>) {
        for child in named_children(node)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
        {
            if Self::is_expression_child(node, child) {
                self.visit(child);
            } else {
                self.visit_type(child);
            }
        }
    }

    fn is_expression_child(wrapper: Node<'_>, child: Node<'_>) -> bool {
        if wrapper.kind() == "type_assertion" {
            child.kind() != "type_arguments"
        } else {
            first_named_child(wrapper).is_some_and(|first| first.id() == child.id())
        }
    }

    fn expression_target(&mut self, node: Node<'tree>) -> Node<'tree> {
        let mut target = node;
        loop {
            target = without_parentheses(target);
            let kind = target.kind();
            if TYPE_WRAPPERS.contains(&kind) || kind == "type_assertion" {
                visit_wrapped_types(self, target);
            }
            let inner = if TYPE_WRAPPERS.contains(&kind) || kind == "non_null_expression" {
                first_named_child(target)
            } else if kind == "type_assertion" {
                named_children(target)
                    .into_iter()
                    .find(|child| child.kind() != "type_arguments" && child.kind() != COMMENT)
            } else {
                None
            };
            match inner {
                Some(inner) => target = inner,
                None => return target,
            }
        }
    }

    fn visit_assignment(&mut self, node: Node<'tree>) {
        if super::bindings::has_token(node, USING) {
            self.visit_using(node);
            return;
        }
        if let Some(left) = node.child_by_field_name("left") {
            assign_target(self, node, left);
        }
        self.visit_field(node, "right");
    }

    fn visit_using(&mut self, node: Node<'tree>) {
        let scope = self.current;
        let declare = |referencer: &mut Self, identifier: Node<'tree>, defaults: usize| {
            referencer.define(scope, identifier, (DefinitionKind::Variable, node, None));
            referencer.write_defaults(identifier, defaults, true);
            referencer.reference(identifier, (Access::Write, Target::Value, true));
        };
        if let Some(name) = node.child_by_field_name("left") {
            self.declare_pattern(name, declare);
        }
        self.visit_field(node, "right");
    }

    fn visit_update(&mut self, node: Node<'tree>) {
        let Some(argument) = node.child_by_field_name("argument") else {
            return;
        };
        let target = without_parentheses(self.expression_target(argument));
        if ["identifier", "undefined"].contains(&target.kind()) {
            self.reference(target, (Access::ReadWrite, Target::Value, false));
        } else {
            self.visit_children(node);
        }
    }

    fn visit_call(&mut self, node: Node<'tree>) {
        for child in named_children(node) {
            if child.kind() == "type_arguments" {
                self.visit_type(child);
            } else {
                self.visit(child);
            }
        }
    }

    fn visit_object(&mut self, node: Node<'tree>) {
        for child in named_children(node) {
            self.visit(child);
        }
    }

    fn visit_computed_key(&mut self, node: Node<'tree>) {
        if let Some(key) = node
            .child_by_field_name("key")
            .filter(|key| key.kind() == "computed_property_name")
            .and_then(first_named_child)
        {
            self.visit(key);
        }
    }

    fn visit_jsx_attribute(&mut self, node: Node<'tree>) {
        for (index, child) in named_children(node).into_iter().enumerate() {
            if index > 0 {
                self.visit(child);
            }
        }
    }

    fn visit_jsx_tag(&mut self, node: Node<'tree>) {
        if node.kind() != "jsx_closing_element" {
            self.reference_jsx_factory();
        }
        if let Some(name) = node.child_by_field_name("name") {
            self.visit_jsx_name(name);
        }
        for child in named_children(node) {
            match child.kind() {
                "type_arguments" => self.visit_type(child),
                "jsx_attribute" | "jsx_expression" => self.visit(child),
                _ => {}
            }
        }
    }

    fn visit_jsx_name(&mut self, name: Node<'tree>) {
        match name.kind() {
            "identifier" => {
                let text = self.text(name);
                let first = text.chars().next();
                let capitalized = first.is_some_and(|first| first.to_uppercase().eq(std::iter::once(first)));
                if capitalized || text == "this" {
                    self.read(name);
                }
            }
            "member_expression" => {
                if let Some(object) = name.child_by_field_name("object") {
                    self.visit_jsx_object(object);
                }
            }
            "jsx_namespace_name" => {
                for part in named_children(name) {
                    self.read(part);
                }
            }
            _ => {}
        }
    }

    fn visit_jsx_object(&mut self, object: Node<'tree>) {
        match object.kind() {
            "identifier" => self.read(object),
            "member_expression" => {
                if let Some(inner) = object.child_by_field_name("object") {
                    self.visit_jsx_object(inner);
                }
            }
            _ => {}
        }
    }

    fn reference_jsx_factory(&mut self) {
        if self.jsx_factory_referenced {
            return;
        }
        let mut current = Some(self.current);
        let mut found = None;
        while let Some(scope) = current.filter(|_| found.is_none()) {
            found = self
                .tree
                .scope(scope)
                .and_then(|candidate| candidate.lookup(JSX_FACTORY))
                .map(|variable| (scope, variable));
            current = self.tree.scope(scope).and_then(|upper| upper.upper);
        }
        let Some((scope, variable)) = found else {
            return;
        };
        let identifier = self
            .tree
            .variable(variable)
            .and_then(|declared| declared.definitions.first())
            .map(|definition| definition.name);
        if let Some(identifier) = identifier {
            self.reference_in(scope, identifier, (Access::Read, Target::Value, false));
        }
        self.jsx_factory_referenced = true;
    }
}

fn visit_wrapped_types<'tree>(referencer: &mut Referencer<'tree, '_, '_>, wrapper: Node<'tree>) {
    for child in named_children(wrapper) {
        if !Referencer::is_expression_child(wrapper, child) && child.kind() != COMMENT {
            referencer.visit_type(child);
        }
    }
}

fn assign_target<'tree>(referencer: &mut Referencer<'tree, '_, '_>, node: Node<'tree>, left: Node<'tree>) {
    let target = referencer.expression_target(left);
    let pattern = without_parentheses(target);
    let simple = node.kind() == "assignment_expression"
        && node
            .child_by_field_name("operator")
            .is_none_or(|operator| referencer.text(operator) == SIMPLE_ASSIGNMENT);
    let is_identifier = ["identifier", "undefined"].contains(&pattern.kind());
    if !PATTERN_TARGETS.contains(&pattern.kind()) {
        referencer.visit(target);
    } else if simple {
        referencer.assign_pattern(pattern);
    } else if is_identifier {
        referencer.reference(pattern, (Access::ReadWrite, Target::Value, false));
    }
}
