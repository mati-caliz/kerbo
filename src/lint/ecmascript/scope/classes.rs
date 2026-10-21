use tree_sitter::Node;

use super::declarations::DeclarationVisitor;
use super::types::TypeVisitor;

use super::DefinitionKind;
use super::ScopeKind;
use super::bindings::ParameterStyle;
use super::referencer::Referencer;
use crate::lint::ecmascript::nodes::COMMENT;
use crate::syntax::named_children;

const DECORATOR: &str = "decorator";
const SETTER_KEYWORD: &str = "set";

pub trait ClassVisitor<'tree> {
    fn visit_class(&mut self, node: Node<'tree>);
    fn visit_superclass(&mut self, heritage: &[Node<'tree>]);
    fn visit_superclass_types(&mut self, heritage: &[Node<'tree>]);
    fn visit_class_body(&mut self, body: Node<'tree>);
    fn visit_method(&mut self, method: Node<'tree>);
    fn visit_field_definition(&mut self, field: Node<'tree>);
}

impl<'tree> ClassVisitor<'tree> for Referencer<'tree, '_, '_> {
    fn visit_class(&mut self, node: Node<'tree>) {
        let name = node.child_by_field_name("name");
        if node.kind() != "class"
            && let Some(name) = name
        {
            let scope = self.current;
            self.define(scope, name, (DefinitionKind::ClassName, node, None));
        }
        for decorator in named_children(node)
            .into_iter()
            .filter(|child| child.kind() == DECORATOR)
        {
            self.visit(decorator);
        }
        self.open(ScopeKind::Class, node);
        if let Some(name) = name {
            let scope = self.current;
            let inner = self.define(scope, name, (DefinitionKind::ClassName, node, None));
            self.mark_used(inner);
        }
        let heritage: Vec<Node<'tree>> = named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "class_heritage")
            .flat_map(named_children)
            .collect();
        self.visit_superclass(&heritage);
        if let Some(parameters) = node.child_by_field_name("type_parameters") {
            self.visit_type(parameters);
        }
        self.visit_superclass_types(&heritage);
        for clause in heritage
            .iter()
            .filter(|clause| clause.kind() == "implements_clause")
        {
            self.visit_type(*clause);
        }
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_class_body(body);
        }
        self.close();
    }

    fn visit_superclass(&mut self, heritage: &[Node<'tree>]) {
        let values = heritage.iter().flat_map(|clause| match clause.kind() {
            "extends_clause" => named_children(*clause)
                .into_iter()
                .filter(|child| child.kind() != "type_arguments" && child.kind() != COMMENT)
                .collect(),
            "implements_clause" | COMMENT => Vec::new(),
            _ => vec![*clause],
        });
        for value in values {
            self.visit(value);
        }
    }

    fn visit_superclass_types(&mut self, heritage: &[Node<'tree>]) {
        for clause in heritage.iter().filter(|clause| clause.kind() == "extends_clause") {
            for arguments in named_children(*clause)
                .into_iter()
                .filter(|child| child.kind() == "type_arguments")
            {
                self.visit_type(arguments);
            }
        }
    }

    fn visit_class_body(&mut self, body: Node<'tree>) {
        for member in named_children(body) {
            match member.kind() {
                "method_definition" => self.visit_method(member),
                "public_field_definition" | "field_definition" => self.visit_field_definition(member),
                "abstract_method_signature" | "method_signature" => {
                    self.visit_computed_name(member);
                    self.function_scope(member, ParameterStyle::USED);
                }
                "class_static_block" => static_block(self, member),
                "index_signature" => self.visit_type(member),
                DECORATOR => self.visit(member),
                _ => {}
            }
        }
    }

    fn visit_method(&mut self, method: Node<'tree>) {
        self.visit_computed_name(method);
        if let Some(parameters) = method.child_by_field_name("parameters") {
            for decorator in named_children(parameters)
                .into_iter()
                .flat_map(named_children)
                .filter(|child| child.kind() == DECORATOR)
            {
                self.visit(decorator);
            }
        }
        let setter = super::bindings::has_token(method, SETTER_KEYWORD);
        self.function_scope(
            method,
            ParameterStyle {
                used: setter,
                decorated_outside: true,
            },
        );
    }

    fn visit_field_definition(&mut self, field: Node<'tree>) {
        self.visit_computed_name(field);
        if let Some(value) = field.child_by_field_name("value") {
            self.open(ScopeKind::ClassFieldInitializer, value);
            self.visit(value);
            self.close();
        }
        if let Some(annotation) = field.child_by_field_name("type") {
            self.visit_type(annotation);
        }
        for decorator in named_children(field)
            .into_iter()
            .filter(|child| child.kind() == DECORATOR)
        {
            self.visit(decorator);
        }
    }
}

fn static_block<'tree>(referencer: &mut Referencer<'tree, '_, '_>, member: Node<'tree>) {
    referencer.open(ScopeKind::ClassStaticBlock, member);
    if let Some(block) = member.child_by_field_name("body") {
        referencer.visit_children(block);
    }
    referencer.close();
}
