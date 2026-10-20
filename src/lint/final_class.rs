mod anonymous;
mod matching;

use std::collections::HashMap;

use tree_sitter::Node;

use super::java::{
    ABSTRACT, BODY_FIELD, CLASS, CLASS_BODY, CONSTRUCTOR, ENUM, FINAL, NAME_FIELD, OBJECT_CREATION, PRIVATE,
    RECORD, children, has_modifier, is_type_declaration, qualified_name, text,
};
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::syntax::named_children;

const PACKAGE: &str = "package_declaration";
const SUPERCLASS_FIELD: &str = "superclass";
const CLASS_KEYWORD: &str = "class";
const FINAL_PREFIX: &str = "final ";
const NAME_SEPARATOR: &str = ".";

#[derive(Debug)]
struct ClassDescription<'tree> {
    node: Node<'tree>,
    qualified: String,
    depth: usize,
    constructors: Constructors,
    is_extended: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Constructors {
    Absent,
    OnlyPrivate,
    NonPrivate,
}

#[derive(Debug)]
struct Scope {
    qualified: String,
    class: Option<usize>,
    owns_constructors: bool,
}

#[derive(Debug, Default)]
struct Collected<'tree> {
    classes: Vec<ClassDescription<'tree>>,
    by_name: HashMap<String, usize>,
    order: Vec<String>,
    anonymous: Vec<(String, String)>,
}

impl<'tree> Collected<'tree> {
    fn register(&mut self, class: ClassDescription<'tree>) -> usize {
        let index = self.classes.len();
        if self.by_name.insert(class.qualified.clone(), index).is_none() {
            self.order.push(class.qualified.clone());
        }
        self.classes.push(class);
        index
    }

    fn current(&self) -> Vec<usize> {
        self.order
            .iter()
            .filter_map(|name| self.by_name.get(name).copied())
            .collect()
    }
}

enum Visit<'tree> {
    Enter(Node<'tree>),
    Leave,
}

pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let mut collected = collect(root, source);
    for (super_name, outer) in std::mem::take(&mut collected.anonymous) {
        let target = matching::nearest(&collected, &super_name, |candidate| {
            matching::anonymous_count(&outer, candidate)
        });
        if let Some(class) = target.and_then(|index| collected.classes.get_mut(index)) {
            class.is_extended = true;
        }
    }
    for index in collected.current() {
        mark_superclass(&mut collected, index, source);
    }
    collected
        .current()
        .into_iter()
        .filter_map(|index| collected.classes.get(index))
        .filter(|class| should_be_final(class))
        .map(|class| {
            let name = class
                .node
                .child_by_field_name(NAME_FIELD)
                .map_or("", |name| text(name, source));
            let keyword = children(class.node)
                .into_iter()
                .find(|token| token.kind() == CLASS_KEYWORD);
            let fix = keyword.map(|keyword| Fix::insert_before(keyword, FINAL_PREFIX, source));
            finding(LintKind::FinalClass, class.node, name, fix)
        })
        .collect()
}

fn mark_superclass(collected: &mut Collected<'_>, index: usize, source: &[u8]) {
    let Some(class) = collected.classes.get(index) else {
        return;
    };
    let Some(superclass) = class.node.child_by_field_name(SUPERCLASS_FIELD) else {
        return;
    };
    let Some(super_name) = superclass
        .named_child(0)
        .and_then(|kind| anonymous::type_name(kind, source))
    else {
        return;
    };
    let subclass = class.qualified.clone();
    let target = matching::nearest(collected, &super_name, |candidate| {
        matching::declaration_count(&subclass, candidate)
    });
    if let Some(extended) = target.and_then(|target| collected.classes.get_mut(target)) {
        extended.is_extended = true;
    }
}

fn should_be_final(class: &ClassDescription<'_>) -> bool {
    let node = class.node;
    if has_modifier(node, FINAL) || has_modifier(node, ABSTRACT) || class.is_extended {
        return false;
    }
    match class.constructors {
        Constructors::Absent => has_modifier(node, PRIVATE),
        Constructors::OnlyPrivate => true,
        Constructors::NonPrivate => false,
    }
}

fn collect<'tree>(root: Node<'tree>, source: &[u8]) -> Collected<'tree> {
    let mut walk = Walk::default();
    let mut pending = vec![Visit::Enter(root)];
    while let Some(visit) = pending.pop() {
        let Visit::Enter(node) = visit else {
            walk.scopes.pop();
            continue;
        };
        match node.kind() {
            PACKAGE => walk.package = qualified_name(node, source),
            CONSTRUCTOR => walk.register_constructor(node),
            OBJECT_CREATION if has_class_body(node) => {
                let super_name = anonymous::super_name(node, source);
                let outer = walk.outer().to_owned();
                walk.collected
                    .anonymous
                    .extend(super_name.map(|name| (name, outer)));
            }
            _ if is_type_declaration(node) => {
                walk.enter_type(node, source);
                pending.push(Visit::Leave);
            }
            _ => {}
        }
        pending.extend(named_children(node).into_iter().rev().map(Visit::Enter));
    }
    walk.collected
}

#[derive(Debug, Default)]
struct Walk<'tree> {
    collected: Collected<'tree>,
    package: String,
    scopes: Vec<Scope>,
}

impl<'tree> Walk<'tree> {
    fn outer(&self) -> &str {
        self.scopes
            .last()
            .map_or(self.package.as_str(), |scope| scope.qualified.as_str())
    }

    fn enter_type(&mut self, node: Node<'tree>, source: &[u8]) {
        let name = node
            .child_by_field_name(NAME_FIELD)
            .map_or("", |name| text(name, source));
        let qualified = [self.outer(), name].join(NAME_SEPARATOR);
        let constructors = if has_constructor(node) {
            Constructors::OnlyPrivate
        } else {
            Constructors::Absent
        };
        let depth = self.scopes.len();
        let class = (node.kind() == CLASS).then(|| {
            self.collected.register(ClassDescription {
                node,
                qualified: qualified.clone(),
                depth,
                constructors,
                is_extended: false,
            })
        });
        self.scopes.push(Scope {
            qualified,
            class,
            owns_constructors: ![ENUM, RECORD].contains(&node.kind()),
        });
    }

    fn register_constructor(&mut self, constructor: Node<'_>) {
        let Some(scope) = self.scopes.last() else {
            return;
        };
        if !scope.owns_constructors || has_modifier(constructor, PRIVATE) {
            return;
        }
        if let Some(class) = scope
            .class
            .and_then(|index| self.collected.classes.get_mut(index))
        {
            class.constructors = Constructors::NonPrivate;
        }
    }
}

fn has_class_body(creation: Node<'_>) -> bool {
    named_children(creation)
        .iter()
        .any(|child| child.kind() == CLASS_BODY)
}

fn has_constructor(class: Node<'_>) -> bool {
    class.child_by_field_name(BODY_FIELD).is_some_and(|body| {
        named_children(body)
            .iter()
            .any(|member| member.kind() == CONSTRUCTOR)
    })
}
