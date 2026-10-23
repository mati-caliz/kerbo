use tree_sitter::Node;

use super::nodes::{COMMENT, STATEMENT_BLOCK, VALUE_FIELD, VARIABLE_DECLARATOR};
use super::paths::Reachability;
use super::token_index::TokenIndex;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const CHECKED: &[&str] = &[
    "break_statement",
    "class_declaration",
    "abstract_class_declaration",
    "continue_statement",
    "debugger_statement",
    "do_statement",
    "expression_statement",
    "for_in_statement",
    "for_statement",
    "if_statement",
    "labeled_statement",
    "return_statement",
    "switch_statement",
    "throw_statement",
    "try_statement",
    "while_statement",
    "with_statement",
    "export_statement",
    LEXICAL_DECLARATION,
];
const LEXICAL_DECLARATION: &str = "lexical_declaration";
const VAR_DECLARATION: &str = "variable_declaration";
const NOT_BLOCK_STATEMENTS: &[&str] = &[
    "class_static_block",
    "internal_module",
    "module",
    "ambient_declaration",
];
const METHOD: &str = "method_definition";
const CONSTRUCTOR: &str = "constructor";
const NAME_FIELD: &str = "name";
const BODY_FIELD: &str = "body";
const CALL: &str = "call_expression";
const FUNCTION_FIELD: &str = "function";
const SUPER: &str = "super";
const CLASS_HERITAGE: &str = "class_heritage";
const IMPLEMENTS: &str = "implements_clause";
const FIELDS: &[&str] = &["field_definition", "public_field_definition"];
const STATIC: &str = "static";

#[derive(Clone, Copy)]
struct Range<'tree> {
    start: Node<'tree>,
    end: Node<'tree>,
}

struct Walker<'context, 'tree> {
    reachability: &'context Reachability,
    tokens: TokenIndex<'tree>,
    source: &'context [u8],
    range: Option<Range<'tree>>,
    constructors: Vec<bool>,
    found: Vec<Node<'tree>>,
}

// `no-unreachable`: código que nunca corre, agrupado en rangos de sentencias seguidas.
pub fn unreachable_code(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mut walker = Walker {
        reachability: context.paths(),
        tokens: TokenIndex::new(context.root),
        source: context.source,
        range: None,
        constructors: Vec::new(),
        found: Vec::new(),
    };
    walker.walk(context.root, None);
    walker.check(None, false);
    walker
        .found
        .into_iter()
        .map(|start| report(EslintRule::NoUnreachable, start, ""))
        .collect()
}

impl<'tree> Walker<'_, 'tree> {
    fn walk(&mut self, node: Node<'tree>, parent: Option<Node<'tree>>) {
        if is_checked(node, parent) {
            let unreachable = !self.reachability.reachable_at_enter(node);
            self.check(Some(node), unreachable);
        }
        let is_constructor = is_constructor(node, self.source);
        if is_constructor {
            self.constructors.push(false);
        }
        if let Some(has_super_call) = self.constructors.last_mut().filter(|_| is_super_call(node)) {
            *has_super_call = true;
        }
        let mut cursor = node.walk();
        let children: Vec<Node<'tree>> = node
            .named_children(&mut cursor)
            .filter(|child| child.kind() != COMMENT)
            .collect();
        for child in children {
            self.walk(child, Some(node));
        }
        if is_constructor {
            self.close_constructor(node, parent);
        }
    }

    // Los campos de una clase derivada cuyo constructor no llama a `super()` no corren nunca.
    fn close_constructor(&mut self, constructor: Node<'tree>, class_body: Option<Node<'tree>>) {
        let has_super_call = self.constructors.pop().unwrap_or(true);
        if has_super_call || constructor.child_by_field_name(BODY_FIELD).is_none() {
            return;
        }
        for field in class_body
            .map(instance_fields_of_derived_class)
            .unwrap_or_default()
        {
            self.check(Some(field), true);
        }
    }

    // `reportIfUnreachable`: junta las sentencias seguidas y avisa una vez por rango.
    fn check(&mut self, node: Option<Node<'tree>>, unreachable: bool) {
        let next = node.filter(|_| unreachable);
        if next.is_some_and(|candidate| self.absorb(candidate)) {
            return;
        }
        if let Some(range) = self.range {
            self.found.push(range.start);
        }
        self.range = next.map(|start| Range { start, end: start });
    }

    // Suma el nodo al rango abierto si empieza uno, ya está adentro o le sigue sin nada en el medio.
    fn absorb(&mut self, node: Node<'tree>) -> bool {
        let Some(range) = self.range else {
            self.range = Some(Range {
                start: node,
                end: node,
            });
            return true;
        };
        if range.start.start_byte() <= node.start_byte() && node.end_byte() <= range.end.end_byte() {
            return true;
        }
        let consecutive = self.tokens.token_before(node.start_byte()).is_some_and(|token| {
            range.start.start_byte() <= token.start && token.end <= range.end.end_byte()
        });
        if consecutive {
            self.range = Some(Range {
                start: range.start,
                end: node,
            });
        }
        consecutive
    }
}

fn is_checked(node: Node<'_>, parent: Option<Node<'_>>) -> bool {
    match node.kind() {
        STATEMENT_BLOCK => parent.is_none_or(|parent| !NOT_BLOCK_STATEMENTS.contains(&parent.kind())),
        VAR_DECLARATION => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor).any(|declarator| {
                declarator.kind() == VARIABLE_DECLARATOR
                    && declarator.child_by_field_name(VALUE_FIELD).is_some()
            })
        }
        kind => CHECKED.contains(&kind),
    }
}

fn is_constructor(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == METHOD
        && node
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| name.utf8_text(source).is_ok_and(|text| text == CONSTRUCTOR))
}

fn is_super_call(node: Node<'_>) -> bool {
    node.kind() == CALL
        && node
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|callee| callee.kind() == SUPER)
}

fn instance_fields_of_derived_class(class_body: Node<'_>) -> Vec<Node<'_>> {
    let derived = class_body.parent().is_some_and(|class| {
        let mut cursor = class.walk();
        class
            .named_children(&mut cursor)
            .any(|child| child.kind() == CLASS_HERITAGE && has_superclass(child))
    });
    if !derived {
        return Vec::new();
    }
    let mut cursor = class_body.walk();
    class_body
        .named_children(&mut cursor)
        .filter(|member| FIELDS.contains(&member.kind()))
        .filter(|field| {
            let mut inner = field.walk();
            let is_static = field.children(&mut inner).any(|child| child.kind() == STATIC);
            !is_static
        })
        .collect()
}

fn has_superclass(heritage: Node<'_>) -> bool {
    let mut cursor = heritage.walk();
    heritage
        .named_children(&mut cursor)
        .any(|clause| clause.kind() != IMPLEMENTS)
}
