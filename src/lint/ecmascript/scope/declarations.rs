use tree_sitter::Node;

use super::classes::ClassVisitor;
use super::patterns::PatternVisitor;
use super::types::TypeVisitor;

use super::bindings::{ParameterStyle, declare_declarator, declare_parameter, has_token, import_specifiers};
use super::referencer::Referencer;
use super::{Access, Definition, DefinitionKind, ScopeKind, Target};
use crate::lint::ecmascript::nodes::{IDENTIFIER, STRING, VARIABLE_DECLARATOR, first_named_child};
use crate::lint::ecmascript::strings::static_string;
use crate::syntax::named_children;

const TYPE_KEYWORD: &str = "type";
const GLOBAL_KEYWORD: &str = "global";
const SETTER_KEYWORD: &str = "set";
const FUNCTION_EXPRESSIONS: &[&str] = &["function_expression", "function", "generator_function"];
const DECLARED_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "function_signature",
];
const ARGUMENTS: &str = "arguments";
const ARROW: &str = "arrow_function";

pub trait DeclarationVisitor<'tree> {
    fn visit_declaration(&mut self, node: Node<'tree>) -> bool;
    fn visit_variables(&mut self, node: Node<'tree>);
    fn visit_for_in(&mut self, node: Node<'tree>);
    fn declare_loop_head(&mut self, statement: Node<'tree>, left: Node<'tree>, lexical: bool);
    fn assign_pattern(&mut self, pattern: Node<'tree>);
    fn visit_function(&mut self, node: Node<'tree>);
    fn visit_computed_name(&mut self, node: Node<'tree>);
    fn function_scope(&mut self, node: Node<'tree>, style: ParameterStyle);
    fn visit_parameters(&mut self, function: Node<'tree>, style: ParameterStyle);
    fn visit_import(&mut self, node: Node<'tree>);
    fn import_clause(&mut self, clause: Node<'tree>, statement: Node<'tree>, type_only: bool);
    fn visit_export(&mut self, node: Node<'tree>);
    fn export_clause(&mut self, clause: Node<'tree>, type_only: bool);
    fn visit_import_alias(&mut self, node: Node<'tree>);
    fn visit_enum(&mut self, node: Node<'tree>);
    fn enum_member(&mut self, member: Node<'tree>);
    fn visit_namespace(&mut self, block: Node<'tree>, name: Option<Node<'tree>>);
    fn visit_ambient(&mut self, node: Node<'tree>);
}

impl<'tree> DeclarationVisitor<'tree> for Referencer<'tree, '_, '_> {
    fn visit_declaration(&mut self, node: Node<'tree>) -> bool {
        match node.kind() {
            "variable_declaration" | "lexical_declaration" => self.visit_variables(node),
            "class_declaration" | "abstract_class_declaration" | "class" => self.visit_class(node),
            "import_statement" => self.visit_import(node),
            "export_statement" => self.visit_export(node),
            "import_alias" => self.visit_import_alias(node),
            "function_signature" => self.visit_function(node),
            "enum_declaration" => self.visit_enum(node),
            "internal_module" | "module" => self.visit_namespace(node, node.child_by_field_name("name")),
            "ambient_declaration" => self.visit_ambient(node),
            "interface_declaration" | "type_alias_declaration" => self.visit_type(node),
            _ => return false,
        }
        true
    }

    fn visit_variables(&mut self, node: Node<'tree>) {
        let target = if node.kind() == "variable_declaration" {
            self.tree.variable_scope_of(self.current)
        } else {
            self.current
        };
        for declarator in named_children(node)
            .into_iter()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        {
            declare_declarator(self, node, declarator, target);
        }
    }

    fn visit_for_in(&mut self, node: Node<'tree>) {
        let kind = node.child_by_field_name("kind").map(|kind| self.text(kind));
        let lexical = kind.is_some_and(|kind| kind != "var");
        if lexical {
            self.open(ScopeKind::For, node);
        }
        if let Some(left) = node.child_by_field_name("left") {
            if kind.is_some() {
                self.declare_loop_head(node, left, lexical);
            } else {
                self.assign_pattern(left);
            }
        }
        self.visit_field(node, "right");
        self.visit_field(node, "body");
        if lexical {
            self.close();
        }
    }

    fn declare_loop_head(&mut self, statement: Node<'tree>, left: Node<'tree>, lexical: bool) {
        let target = if lexical {
            self.current
        } else {
            self.tree.variable_scope_of(self.current)
        };
        self.declare_pattern(left, |referencer, identifier, defaults| {
            referencer.define(
                target,
                identifier,
                (DefinitionKind::Variable, left, Some(statement)),
            );
            referencer.write_defaults(identifier, defaults, true);
        });
        for identifier in super::patterns::pattern_parts(left).identifiers {
            self.reference(identifier.node, (Access::Write, Target::Value, true));
        }
    }

    fn assign_pattern(&mut self, pattern: Node<'tree>) {
        self.declare_pattern(pattern, |referencer, identifier, defaults| {
            referencer.write_defaults(identifier, defaults, false);
            referencer.reference(identifier, (Access::Write, Target::Value, false));
        });
    }

    fn visit_function(&mut self, node: Node<'tree>) {
        let kind = node.kind();
        let name = node.child_by_field_name("name");
        let mut named_expression = false;
        if FUNCTION_EXPRESSIONS.contains(&kind) {
            if let Some(name) = name {
                self.open(ScopeKind::FunctionExpressionName, node);
                let scope = self.current;
                self.define(scope, name, (DefinitionKind::FunctionName, node, None));
                named_expression = true;
            }
        } else if DECLARED_FUNCTIONS.contains(&kind) {
            if let Some(name) = name {
                let scope = self.current;
                self.define(scope, name, (DefinitionKind::FunctionName, node, None));
            }
        } else if kind == "method_definition" {
            self.visit_computed_name(node);
        }
        let used = kind == "function_signature" || has_token(node, SETTER_KEYWORD);
        self.function_scope(
            node,
            ParameterStyle {
                used,
                decorated_outside: false,
            },
        );
        if named_expression {
            self.close();
        }
    }

    fn visit_computed_name(&mut self, node: Node<'tree>) {
        if let Some(key) = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("property"))
            .filter(|name| name.kind() == "computed_property_name")
            .and_then(first_named_child)
        {
            self.visit(key);
        }
    }

    fn function_scope(&mut self, node: Node<'tree>, style: ParameterStyle) {
        self.open(ScopeKind::Function, node);
        if node.kind() != ARROW {
            let scope = self.current;
            self.tree.define(scope, ARGUMENTS, None);
        }
        self.visit_parameters(node, style);
        for field in ["return_type", "type_parameters"] {
            if let Some(child) = node.child_by_field_name(field) {
                self.visit_type(child);
            }
        }
        if let Some(body) = node.child_by_field_name("body") {
            if body.kind() == "statement_block" {
                self.visit_children(body);
            } else {
                self.visit(body);
            }
        }
        self.close();
    }

    fn visit_parameters(&mut self, function: Node<'tree>, style: ParameterStyle) {
        let parameters = function.child_by_field_name("parameters").map_or_else(
            || function.child_by_field_name("parameter").into_iter().collect(),
            named_children,
        );
        for parameter in parameters.into_iter().filter(|child| !Self::is_comment(*child)) {
            declare_parameter(self, function, parameter, style);
        }
    }

    fn visit_import(&mut self, node: Node<'tree>) {
        let type_only = has_token(node, TYPE_KEYWORD);
        let scope = self.current;
        let children = named_children(node);
        for clause in children.iter().filter(|child| child.kind() == "import_clause") {
            self.import_clause(*clause, node, type_only);
        }
        let required = children
            .iter()
            .filter(|child| child.kind() == "import_require_clause")
            .find_map(|clause| {
                named_children(*clause)
                    .into_iter()
                    .find(|part| part.kind() == IDENTIFIER)
            });
        if let Some(identifier) = required {
            let kind = DefinitionKind::ImportBinding { type_only };
            self.define(scope, identifier, (kind, node, Some(node)));
        }
    }

    fn import_clause(&mut self, clause: Node<'tree>, statement: Node<'tree>, type_only: bool) {
        let scope = self.current;
        let kind = DefinitionKind::ImportBinding { type_only };
        for part in named_children(clause) {
            let binding = match part.kind() {
                IDENTIFIER => Some(part),
                "namespace_import" => first_named_child(part),
                "named_imports" => {
                    import_specifiers(self, part, statement, type_only);
                    None
                }
                _ => None,
            };
            if let Some(identifier) = binding {
                self.define(scope, identifier, (kind, part, Some(statement)));
            }
        }
    }

    fn visit_export(&mut self, node: Node<'tree>) {
        for decorator in named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "decorator")
        {
            self.visit(decorator);
        }
        if let Some(declaration) = node.child_by_field_name("declaration") {
            self.visit(declaration);
            return;
        }
        if node.child_by_field_name("source").is_some() {
            return;
        }
        let type_only = has_token(node, TYPE_KEYWORD);
        for child in named_children(node) {
            match child.kind() {
                "export_clause" => self.export_clause(child, type_only),
                IDENTIFIER => self.reference(child, (Access::Read, Target::Both, false)),
                "decorator" | "namespace_export" | "comment" => {}
                _ => self.visit(child),
            }
        }
    }

    fn export_clause(&mut self, clause: Node<'tree>, type_only: bool) {
        for specifier in named_children(clause)
            .into_iter()
            .filter(|child| child.kind() == "export_specifier")
        {
            let Some(local) = specifier
                .child_by_field_name("name")
                .filter(|local| local.kind() == IDENTIFIER)
            else {
                continue;
            };
            let target = if type_only || has_token(specifier, TYPE_KEYWORD) {
                Target::Type
            } else {
                Target::Both
            };
            self.reference(local, (Access::Read, target, false));
        }
    }

    fn visit_import_alias(&mut self, node: Node<'tree>) {
        let parts: Vec<Node<'tree>> = named_children(node)
            .into_iter()
            .filter(|child| !Self::is_comment(*child))
            .collect();
        let scope = self.current;
        if let Some(name) = parts.first() {
            self.define(
                scope,
                *name,
                (
                    DefinitionKind::ImportBinding { type_only: false },
                    node,
                    Some(node),
                ),
            );
        }
        if let Some(mut reference) = parts.get(1).copied() {
            while let Some(object) = reference.child_by_field_name("object") {
                reference = object;
            }
            self.visit(reference);
        }
    }

    fn visit_enum(&mut self, node: Node<'tree>) {
        let scope = self.current;
        if let Some(name) = node.child_by_field_name("name") {
            self.define(scope, name, (DefinitionKind::EnumName, node, None));
        }
        self.open(ScopeKind::Enum, node);
        if let Some(body) = node.child_by_field_name("body") {
            for member in named_children(body) {
                self.enum_member(member);
            }
        }
        self.close();
    }

    fn enum_member(&mut self, member: Node<'tree>) {
        let (name, value) = if member.kind() == "enum_assignment" {
            (
                member.child_by_field_name("name"),
                member.child_by_field_name("value"),
            )
        } else {
            (Some(member), None)
        };
        let scope = self.current;
        let variable = name.and_then(|name| match name.kind() {
            "property_identifier" => {
                Some(self.define(scope, name, (DefinitionKind::EnumMember, member, None)))
            }
            STRING => static_string(name, self.source).map(|text| {
                let definition = Definition {
                    kind: DefinitionKind::EnumMember,
                    name,
                    node: member,
                    declaration: None,
                };
                self.define_named(scope, &text, definition)
            }),
            _ => None,
        });
        if let Some(variable) = variable {
            self.mark_used(variable);
        }
        if let Some(value) = value {
            self.visit(value);
        }
    }

    fn visit_namespace(&mut self, block: Node<'tree>, name: Option<Node<'tree>>) {
        if let Some(name) = name.filter(|name| name.kind() == IDENTIFIER) {
            let scope = self.current;
            self.define(scope, name, (DefinitionKind::ModuleName, block, None));
        }
        self.open(ScopeKind::Namespace, block);
        if let Some(body) = block.child_by_field_name("body").or_else(|| {
            named_children(block)
                .into_iter()
                .find(|child| child.kind() == "statement_block")
        }) {
            self.visit_children(body);
        }
        self.close();
    }

    fn visit_ambient(&mut self, node: Node<'tree>) {
        if has_token(node, GLOBAL_KEYWORD) {
            self.visit_namespace(node, None);
        } else {
            self.visit_children(node);
        }
    }
}
