use std::collections::HashMap;

use tree_sitter::Node;

use super::{ShortName, field, has_token, named_children, short_name, text};

const MIN_LENGTH: usize = 3;
const MAX_DISTANCE: usize = 5;
const BLANK: &str = "_";
const CONVENTIONAL: &[(&str, &str)] = &[
    ("ctx", "context.Context"),
    ("b", "*testing.B"),
    ("f", "*testing.F"),
    ("m", "*testing.M"),
    ("pb", "*testing.PB"),
    ("t", "*testing.T"),
    ("tb", "testing.TB"),
];
const IDENTIFIER: &str = "identifier";
const SHORT_DECLARATION: &str = ":=";
const SPECS: &[&str] = &["var_spec", "const_spec"];
const SPEC_LISTS: &[&str] = &["var_declaration", "const_declaration", "var_spec_list"];
const TOP_LEVEL_NAMED: &[&str] = &["function_declaration", "type_declaration"];
const FUNCTIONS: &[&str] = &["function_declaration", "method_declaration", "func_literal"];
const SCOPED: &[&str] = &[
    "block",
    "if_statement",
    "for_statement",
    "expression_switch_statement",
    "select_statement",
    "expression_case",
    "default_case",
    "type_case",
    "communication_case",
];
const TYPE_SWITCH: &str = "type_switch_statement";
const TYPE_CASES: &[&str] = &["type_case", "default_case"];
const RANGE: &str = "range_clause";
const RECEIVE: &str = "receive_statement";
const KEYED_ELEMENT: &str = "keyed_element";
const COMPOSITE_LITERAL: &str = "composite_literal";
const MAP_TYPE: &str = "map_type";
const LABELED: &str = "labeled_statement";
const TYPE_PARAMETER: &str = "type_parameter_declaration";
const PARAMETERS: &[&str] = &["parameter_declaration", "variadic_parameter_declaration"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Checked,
    Unchecked,
}

struct Variable<'tree> {
    name: &'tree str,
    declared: Node<'tree>,
    last_row: usize,
    kind: Kind,
}

struct Resolver<'tree> {
    source: &'tree [u8],
    scopes: Vec<HashMap<&'tree str, usize>>,
    variables: Vec<Variable<'tree>>,
}

pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<ShortName> {
    let mut resolver = Resolver {
        source,
        scopes: vec![HashMap::new()],
        variables: Vec::new(),
    };
    resolver.declare_top_level(root);
    resolver.visit_children(root);
    resolver
        .variables
        .iter()
        .filter(|variable| {
            variable.kind == Kind::Checked
                && variable.name.len() < MIN_LENGTH
                && variable
                    .last_row
                    .saturating_sub(variable.declared.start_position().row)
                    > MAX_DISTANCE
        })
        .map(|variable| short_name(variable.declared, variable.name))
        .collect()
}

impl<'tree> Resolver<'tree> {
    fn declare_top_level(&mut self, root: Node<'tree>) {
        for declaration in named_children(root) {
            if SPEC_LISTS.contains(&declaration.kind()) {
                self.declare_specs(declaration);
            } else if TOP_LEVEL_NAMED.contains(&declaration.kind()) {
                let declared: Vec<Node<'tree>> = named_children(declaration)
                    .iter()
                    .filter_map(|spec| field(*spec, "name"))
                    .chain(field(declaration, "name"))
                    .collect();
                self.declare_all(declared, declaration, Kind::Unchecked);
            }
        }
    }

    fn declare_specs(&mut self, declaration: Node<'tree>) {
        for spec in specs(declaration) {
            self.declare_all(names(spec), spec, Kind::Checked);
        }
    }

    fn declare_all(&mut self, declared_names: Vec<Node<'tree>>, declared: Node<'tree>, kind: Kind) {
        for name in declared_names {
            self.declare(name, declared, kind);
        }
    }

    fn declare(&mut self, name: Node<'tree>, declared: Node<'tree>, kind: Kind) -> Option<usize> {
        let name_text = text(name, self.source);
        if name_text == BLANK || name_text.is_empty() {
            return None;
        }
        let index = self.variables.len();
        self.variables.push(Variable {
            name: name_text,
            declared,
            last_row: name.start_position().row,
            kind,
        });
        self.scopes.last_mut()?.insert(name_text, index);
        Some(index)
    }

    fn use_name(&mut self, name: Node<'tree>) {
        let name_text = text(name, self.source);
        let found = self
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name_text).copied());
        if let Some(variable) = found.and_then(|index| self.variables.get_mut(index)) {
            variable.last_row = variable.last_row.max(name.start_position().row);
        }
    }

    fn scoped(&mut self, body: impl FnOnce(&mut Self)) {
        self.scopes.push(HashMap::new());
        body(self);
        self.scopes.pop();
    }

    fn visit_children(&mut self, node: Node<'tree>) {
        for child in named_children(node) {
            self.visit(child);
        }
    }

    fn visit(&mut self, node: Node<'tree>) {
        let kind = node.kind();
        match kind {
            IDENTIFIER => self.use_name(node),
            kind if FUNCTIONS.contains(&kind) => self.scoped(|resolver| resolver.function(node)),
            kind if SCOPED.contains(&kind) => self.scoped(|resolver| resolver.visit_children(node)),
            TYPE_SWITCH => self.scoped(|resolver| resolver.type_switch(node)),
            "short_var_declaration" => self.short_declaration(node),
            RANGE | RECEIVE if has_token(node, SHORT_DECLARATION) => self.short_declaration(node),
            kind if SPEC_LISTS.contains(&kind) && self.scopes.len() > 1 => self.local_specs(node),
            kind if SPECS.contains(&kind) => self.spec_values(node),
            KEYED_ELEMENT => self.keyed_element(node),
            LABELED => {
                if let Some(statement) = named_children(node).into_iter().nth(1) {
                    self.visit(statement);
                }
            }
            _ => self.visit_children(node),
        }
    }

    fn function(&mut self, node: Node<'tree>) {
        for (list_field, kind) in [
            ("receiver", Kind::Unchecked),
            ("type_parameters", Kind::Unchecked),
            ("parameters", Kind::Checked),
            ("result", Kind::Unchecked),
        ] {
            if let Some(list) = field(node, list_field) {
                self.declare_parameters(list, kind);
            }
        }
        if let Some(body) = field(node, "body") {
            self.visit(body);
        }
    }

    fn declare_parameters(&mut self, list: Node<'tree>, kind: Kind) {
        let parameters = named_children(list)
            .into_iter()
            .filter(|parameter| PARAMETERS.contains(&parameter.kind()) || parameter.kind() == TYPE_PARAMETER);
        for parameter in parameters {
            if let Some(parameter_type) = field(parameter, "type") {
                self.visit(parameter_type);
            }
            let checked = kind == Kind::Checked && !self.is_conventional(parameter);
            let parameter_kind = if checked { Kind::Checked } else { Kind::Unchecked };
            self.declare_all(names(parameter), parameter, parameter_kind);
        }
    }

    fn is_conventional(&self, parameter: Node<'tree>) -> bool {
        let declared_type: String = field(parameter, "type")
            .map(|parameter_type| text(parameter_type, self.source))
            .unwrap_or_default()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        names(parameter).iter().all(|name| {
            CONVENTIONAL.iter().any(|&(conventional, typed)| {
                text(*name, self.source) == conventional && declared_type == typed
            })
        })
    }

    fn short_declaration(&mut self, node: Node<'tree>) {
        if let Some(values) = field(node, "right") {
            self.visit(values);
        }
        let targets = field(node, "left").map(named_children).unwrap_or_default();
        for target in targets.into_iter().filter(|target| target.kind() == IDENTIFIER) {
            let already = self
                .scopes
                .last()
                .is_some_and(|scope| scope.contains_key(text(target, self.source)));
            if already {
                self.use_name(target);
            } else {
                self.declare(target, node, Kind::Checked);
            }
        }
    }

    fn local_specs(&mut self, declaration: Node<'tree>) {
        for spec in specs(declaration) {
            self.spec_values(spec);
            self.declare_all(names(spec), spec, Kind::Checked);
        }
    }

    fn spec_values(&mut self, spec: Node<'tree>) {
        for part in ["type", "value"] {
            if let Some(value) = field(spec, part) {
                self.visit(value);
            }
        }
        if self.scopes.len() == 1 {
            for name in names(spec) {
                self.use_name(name);
            }
        }
    }

    fn type_switch(&mut self, node: Node<'tree>) {
        for part in ["initializer", "value"] {
            if let Some(child) = field(node, part) {
                self.visit(child);
            }
        }
        let aliases = field(node, "alias").map(named_children).unwrap_or_default();
        for case in named_children(node)
            .into_iter()
            .filter(|case| TYPE_CASES.contains(&case.kind()))
        {
            self.scoped(|resolver| {
                resolver.declare_aliases(&aliases);
                resolver.visit_children(case);
            });
        }
    }

    fn declare_aliases(&mut self, aliases: &[Node<'tree>]) {
        let Some(&first) = aliases.first() else {
            return;
        };
        for &alias in aliases {
            self.declare_shared(alias, first);
        }
    }

    fn declare_shared(&mut self, alias: Node<'tree>, declared: Node<'tree>) {
        let name_text = text(alias, self.source);
        let existing = self
            .variables
            .iter()
            .position(|variable| variable.declared.id() == declared.id() && variable.name == name_text);
        match existing {
            Some(index) => {
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name_text, index);
                }
            }
            None => {
                self.declare(alias, declared, Kind::Checked);
            }
        }
    }

    fn keyed_element(&mut self, element: Node<'tree>) {
        let in_map = element
            .parent()
            .and_then(|literal| literal.parent())
            .filter(|composite| composite.kind() == COMPOSITE_LITERAL)
            .and_then(|composite| field(composite, "type"))
            .is_some_and(|literal_type| literal_type.kind() == MAP_TYPE);
        if in_map && let Some(key) = field(element, "key") {
            self.visit(key);
        }
        if let Some(value) = field(element, "value") {
            self.visit(value);
        }
    }
}

fn specs(declaration: Node<'_>) -> Vec<Node<'_>> {
    named_children(declaration)
        .into_iter()
        .flat_map(|child| {
            if SPECS.contains(&child.kind()) {
                vec![child]
            } else if SPEC_LISTS.contains(&child.kind()) {
                specs(child)
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn names(declaration: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = declaration.walk();
    declaration
        .children_by_field_name("name", &mut cursor)
        .filter(|name| name.kind() == IDENTIFIER)
        .collect()
}
