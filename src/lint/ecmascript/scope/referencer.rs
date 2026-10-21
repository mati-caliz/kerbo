use tree_sitter::Node;

use super::declarations::DeclarationVisitor;
use super::expressions::ExpressionVisitor;
use super::patterns::PatternVisitor;
use super::types::TypeVisitor;

use super::{
    Access, Definition, DefinitionKind, GLOBAL, Reference, ScopeId, ScopeKind, ScopeTree, Target, VariableId,
};
use crate::lint::ecmascript::nodes::{COMMENT, is_function, text};
use crate::syntax::named_children;

pub const TYPE_ENTRY_KINDS: &[&str] = &[
    "type_annotation",
    "type_arguments",
    "type_parameters",
    "asserts_annotation",
    "type_predicate_annotation",
];
const IGNORED: &[&str] = &[
    COMMENT,
    "hash_bang_line",
    "property_identifier",
    "private_property_identifier",
    "statement_identifier",
    "type_identifier",
    "predefined_type",
    "break_statement",
    "continue_statement",
    "meta_property",
    "this",
    "super",
    "import",
    "regex",
    "string",
    "number",
];
const REFERENCES: &[&str] = &["identifier", "shorthand_property_identifier", "undefined"];

pub struct Referencer<'tree, 'scopes, 'source> {
    pub tree: &'scopes mut ScopeTree<'tree>,
    pub source: &'source [u8],
    pub current: ScopeId,
    pub jsx_factory_referenced: bool,
}

impl<'tree, 'scopes, 'source> Referencer<'tree, 'scopes, 'source> {
    pub const fn new(tree: &'scopes mut ScopeTree<'tree>, source: &'source [u8]) -> Self {
        Self {
            tree,
            source,
            current: GLOBAL,
            jsx_factory_referenced: false,
        }
    }

    pub fn program(&mut self, root: Node<'tree>) {
        self.current = self.tree.open(ScopeKind::Global, root, None);
        self.open(ScopeKind::Module, root);
        self.visit_children(root);
        self.close();
        self.tree.close(GLOBAL, self.source);
    }

    pub fn text(&self, node: Node<'_>) -> &'source str {
        text(node, self.source)
    }

    pub fn open(&mut self, kind: ScopeKind, block: Node<'tree>) {
        self.current = self.tree.open(kind, block, Some(self.current));
    }

    pub fn close(&mut self) {
        let closing = self.current;
        self.tree.close(closing, self.source);
        self.current = self
            .tree
            .scope(closing)
            .and_then(|scope| scope.upper)
            .unwrap_or(GLOBAL);
    }

    pub fn define(
        &mut self,
        scope: ScopeId,
        name: Node<'tree>,
        site: (DefinitionKind, Node<'tree>, Option<Node<'tree>>),
    ) -> VariableId {
        let (kind, node, declaration) = site;
        let definition = Definition {
            kind,
            name,
            node,
            declaration,
        };
        self.tree.define(scope, self.text(name), Some(definition))
    }

    pub fn define_named(&mut self, scope: ScopeId, name: &str, definition: Definition<'tree>) -> VariableId {
        self.tree.define(scope, name, Some(definition))
    }

    pub fn mark_used(&mut self, variable: VariableId) {
        if let Some(found) = self.tree.variables.get_mut(variable) {
            found.used = true;
        }
    }

    pub fn reference(&mut self, identifier: Node<'tree>, flags: (Access, Target, bool)) {
        self.reference_in(self.current, identifier, flags);
    }

    pub fn reference_in(&mut self, scope: ScopeId, identifier: Node<'tree>, flags: (Access, Target, bool)) {
        let (access, target, init) = flags;
        self.tree.reference_from(
            scope,
            Reference {
                identifier,
                from: scope,
                access,
                target,
                init,
                resolved: None,
            },
        );
    }

    pub fn read(&mut self, identifier: Node<'tree>) {
        self.reference(identifier, (Access::Read, Target::Value, false));
    }

    pub fn visit_children(&mut self, node: Node<'tree>) {
        for child in named_children(node) {
            self.visit(child);
        }
    }

    pub fn visit_field(&mut self, node: Node<'tree>, field: &str) {
        if let Some(child) = node.child_by_field_name(field) {
            self.visit(child);
        }
    }

    pub fn visit(&mut self, node: Node<'tree>) {
        let kind = node.kind();
        if IGNORED.contains(&kind) || !node.is_named() {
            return;
        }
        if REFERENCES.contains(&kind) {
            self.read(node);
        } else if TYPE_ENTRY_KINDS.contains(&kind) {
            self.visit_type(node);
        } else if is_function(node) {
            self.visit_function(node);
        } else if !self.visit_declaration(node) && !self.visit_statement(node) && !self.visit_expression(node)
        {
            self.visit_children(node);
        }
    }

    fn visit_statement(&mut self, node: Node<'tree>) -> bool {
        match node.kind() {
            "statement_block" => self.visit_block(node),
            "for_statement" => self.visit_for(node),
            "for_in_statement" => self.visit_for_in(node),
            "switch_statement" => self.visit_switch(node),
            "catch_clause" => self.visit_catch(node),
            "with_statement" => self.visit_with(node),
            "labeled_statement" => self.visit_field(node, "body"),
            _ => return false,
        }
        true
    }

    pub fn visit_block(&mut self, node: Node<'tree>) {
        self.open(ScopeKind::Block, node);
        self.visit_children(node);
        self.close();
    }

    fn visit_for(&mut self, node: Node<'tree>) {
        let lexical = node
            .child_by_field_name("initializer")
            .is_some_and(|initializer| initializer.kind() == "lexical_declaration");
        if lexical {
            self.open(ScopeKind::For, node);
        }
        self.visit_children(node);
        if lexical {
            self.close();
        }
    }

    fn visit_switch(&mut self, node: Node<'tree>) {
        self.visit_field(node, "value");
        self.open(ScopeKind::Switch, node);
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_children(body);
        }
        self.close();
    }

    fn visit_catch(&mut self, node: Node<'tree>) {
        self.open(ScopeKind::Catch, node);
        if let Some(parameter) = node.child_by_field_name("parameter") {
            let scope = self.current;
            self.declare_pattern(parameter, |referencer, identifier, defaults| {
                referencer.define(scope, identifier, (DefinitionKind::CatchClause, node, None));
                referencer.write_defaults(identifier, defaults, true);
            });
        }
        self.visit_field(node, "body");
        self.close();
    }

    fn visit_with(&mut self, node: Node<'tree>) {
        self.visit_field(node, "object");
        self.open(ScopeKind::With, node);
        self.visit_field(node, "body");
        self.close();
    }

    pub fn write_defaults(&mut self, identifier: Node<'tree>, defaults: usize, init: bool) {
        for _ in 0..defaults {
            self.reference(identifier, (Access::Write, Target::Value, init));
        }
    }

    pub fn is_comment(node: Node<'_>) -> bool {
        node.kind() == COMMENT
    }
}
