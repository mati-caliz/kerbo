use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::nodes::{
    ARGUMENTS, ARGUMENTS_FIELD, ASSIGNMENTS, CALL, FUNCTION_FIELD, IDENTIFIER, INDEX_FIELD, LEFT_FIELD,
    MEMBER, OBJECT_FIELD, PROPERTY_FIELD, SUBSCRIPT, VALUE_FIELD, first_named_child, operator,
    parent_without_parentheses, text, without_parentheses,
};
use super::scope::{DefinitionKind, Reference, ScopeKind, ScopeTree, Variable};
use super::strings::static_string;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const KIND_FIELD: &str = "kind";
const CONSTANT_BINDINGS: &[&str] = &["const", "using", "await using"];
const UPDATE: &str = "update_expression";
const UNARY: &str = "unary_expression";
const DELETE: &str = "delete";
const FOR_IN: &str = "for_in_statement";
const NAMESPACE_IMPORT: &str = "namespace_import";
const NOT_IMPORT_DECLARATIONS: &[&str] = &["import_require_clause", "import_alias"];
const WRITE_NODES: &[&str] = &[
    "assignment_expression",
    "augmented_assignment_expression",
    UPDATE,
    UNARY,
    CALL,
    FOR_IN,
];
const ASSIGNMENT_PATTERN: &str = "assignment_pattern";
const PATTERN_PARENTS: &[&str] = &["array_pattern", "rest_pattern"];
const PAIR_PATTERN: &str = "pair_pattern";
const OBJECT_GLOBAL: &str = "Object";
const REFLECT_GLOBAL: &str = "Reflect";
pub const READONLY_GLOBALS: &[&str] = &[
    "Array",
    "Boolean",
    "constructor",
    "Date",
    "decodeURI",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    "Error",
    "escape",
    "eval",
    "EvalError",
    "Function",
    "hasOwnProperty",
    "Infinity",
    "isFinite",
    "isNaN",
    "isPrototypeOf",
    "Math",
    "NaN",
    "Number",
    "Object",
    "parseFloat",
    "parseInt",
    "propertyIsEnumerable",
    "RangeError",
    "ReferenceError",
    "RegExp",
    "String",
    "SyntaxError",
    "toLocaleString",
    "toString",
    "TypeError",
    "undefined",
    "unescape",
    "URIError",
    "valueOf",
    "JSON",
    "ArrayBuffer",
    "DataView",
    "Float32Array",
    "Float64Array",
    "Int16Array",
    "Int32Array",
    "Int8Array",
    "Intl",
    "Map",
    "Promise",
    "Proxy",
    "Reflect",
    "Set",
    "Symbol",
    "Uint16Array",
    "Uint32Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "WeakMap",
    "WeakSet",
    "Atomics",
    "SharedArrayBuffer",
    "BigInt",
    "BigInt64Array",
    "BigUint64Array",
    "globalThis",
    "AggregateError",
    "FinalizationRegistry",
    "WeakRef",
    "Float16Array",
    "Iterator",
    "AsyncDisposableStack",
    "DisposableStack",
    "SuppressedError",
    "Temporal",
];

static OBJECT_MUTATIONS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new("^(?:assign|definePropert(?:y|ies)|freeze|setPrototypeOf)$").ok());
static REFLECT_MUTATIONS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new("^(?:(?:define|delete)Property|set(?:PrototypeOf)?)$").ok());

// `no-const-assign`: lo que se le asigna a una constante (`const`, `using`) después de declararla.
pub fn constant_assignments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    modifying_references_of(context, EslintRule::NoConstAssign, |variable, source| {
        variable.definitions.iter().any(|definition| {
            definition.kind == DefinitionKind::Variable
                && definition
                    .declaration
                    .and_then(|declaration| declaration.child_by_field_name(KIND_FIELD))
                    .is_some_and(|kind| CONSTANT_BINDINGS.contains(&text(kind, source)))
        })
    })
}

// `no-class-assign`: lo que se le asigna al nombre de una clase, afuera o adentro de ella.
pub fn class_assignments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    modifying_references_of(context, EslintRule::NoClassAssign, |variable, _| {
        variable
            .definitions
            .iter()
            .any(|definition| definition.kind == DefinitionKind::ClassName)
    })
}

fn modifying_references_of(
    context: &Context<'_, '_>,
    rule: EslintRule,
    is_checked: impl Fn(&Variable<'_>, &[u8]) -> bool,
) -> Vec<LintFinding> {
    let tree = context.scopes();
    tree.variables
        .iter()
        .filter(|variable| is_checked(variable, context.source))
        .flat_map(|variable| modifying_references(tree, variable))
        .map(|reference| report(rule, reference.identifier, context.text(reference.identifier)))
        .collect()
}

// `getModifyingReferences`: las escrituras que no inicializan, una por identificador.
fn modifying_references<'scope, 'tree>(
    tree: &'scope ScopeTree<'tree>,
    variable: &'scope Variable<'tree>,
) -> Vec<&'scope Reference<'tree>> {
    let mut previous: Option<usize> = None;
    let mut found = Vec::new();
    for reference in tree.references_of(variable) {
        let identifier = reference.identifier.id();
        if !reference.init && reference.is_write() && previous != Some(identifier) {
            found.push(reference);
        }
        previous = Some(identifier);
    }
    found
}

// `no-global-assign`: una escritura a un global de ECMAScript, que es de sólo lectura.
pub fn global_assignments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let mut previous: Option<usize> = None;
    let mut found = Vec::new();
    for reference in tree
        .references
        .iter()
        .filter(|reference| reference.resolved.is_none() && reference.is_value())
        .filter(|reference| READONLY_GLOBALS.contains(&context.text(reference.identifier)))
    {
        let identifier = reference.identifier.id();
        if !reference.init && reference.is_write() && previous != Some(identifier) {
            found.push(report(
                EslintRule::NoGlobalAssign,
                reference.identifier,
                context.text(reference.identifier),
            ));
        }
        previous = Some(identifier);
    }
    found
}

// `no-import-assign`: lo que pisa un import, o un miembro de un `import * as`.
pub fn import_assignments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let mut found = Vec::new();
    for variable in tree.variables.iter().filter(|variable| is_imported(variable)) {
        let namespace = variable.definitions.iter().any(|definition| {
            definition
                .name
                .parent()
                .is_some_and(|parent| parent.kind() == NAMESPACE_IMPORT)
        });
        let mut previous: Option<usize> = None;
        for reference in tree.references_of(variable) {
            let identifier = reference.identifier;
            if previous == Some(identifier.id()) {
                continue;
            }
            previous = Some(identifier.id());
            if reference.is_write() || (namespace && is_member_write(identifier, tree, context)) {
                found.push(report(
                    EslintRule::NoImportAssign,
                    write_node(identifier),
                    context.text(identifier),
                ));
            }
        }
    }
    found
}

pub fn is_imported(variable: &Variable<'_>) -> bool {
    variable.definitions.iter().any(|definition| {
        matches!(definition.kind, DefinitionKind::ImportBinding { .. })
            && definition
                .name
                .parent()
                .is_none_or(|parent| !NOT_IMPORT_DECLARATIONS.contains(&parent.kind()))
    })
}

fn write_node(identifier: Node<'_>) -> Node<'_> {
    let mut current = identifier.parent();
    while let Some(node) = current {
        if WRITE_NODES.contains(&node.kind()) {
            return node;
        }
        current = node.parent();
    }
    identifier
}

fn is_member_write(identifier: Node<'_>, tree: &ScopeTree<'_>, context: &Context<'_, '_>) -> bool {
    let member = parent_without_parentheses(identifier).filter(|parent| {
        [MEMBER, SUBSCRIPT].contains(&parent.kind())
            && parent
                .child_by_field_name(OBJECT_FIELD)
                .map(without_parentheses)
                .is_some_and(|object| object.id() == identifier.id())
    });
    if let Some(member) = member {
        return is_assignment_target(member)
            || is_mutated_operand(member, context)
            || is_iteration_target(member);
    }
    is_mutation_function_argument(identifier, tree, context)
}

fn is_assignment_target(member: Node<'_>) -> bool {
    let Some(parent) = parent_without_parentheses(member) else {
        return false;
    };
    if ASSIGNMENTS.contains(&parent.kind()) || parent.kind() == ASSIGNMENT_PATTERN {
        return parent
            .child_by_field_name(LEFT_FIELD)
            .map(without_parentheses)
            .is_some_and(|left| left.id() == member.id());
    }
    if parent.kind() == PAIR_PATTERN {
        return parent
            .child_by_field_name(VALUE_FIELD)
            .map(without_parentheses)
            .is_some_and(|value| value.id() == member.id());
    }
    PATTERN_PARENTS.contains(&parent.kind())
}

fn is_mutated_operand(member: Node<'_>, context: &Context<'_, '_>) -> bool {
    parent_without_parentheses(member).is_some_and(|parent| {
        parent.kind() == UPDATE || (parent.kind() == UNARY && operator(parent, context.source) == DELETE)
    })
}

fn is_iteration_target(member: Node<'_>) -> bool {
    parent_without_parentheses(member).is_some_and(|parent| {
        parent.kind() == FOR_IN
            && parent
                .child_by_field_name(LEFT_FIELD)
                .map(without_parentheses)
                .is_some_and(|left| left.id() == member.id())
    })
}

fn is_mutation_function_argument(
    identifier: Node<'_>,
    tree: &ScopeTree<'_>,
    context: &Context<'_, '_>,
) -> bool {
    let Some(arguments) = parent_without_parentheses(identifier).filter(|parent| parent.kind() == ARGUMENTS)
    else {
        return false;
    };
    let is_first = first_named_child(arguments)
        .map(without_parentheses)
        .is_some_and(|first| first.id() == identifier.id());
    let Some(callee) = arguments
        .parent()
        .filter(|call| call.kind() == CALL && is_first)
        .filter(|call| {
            call.child_by_field_name(ARGUMENTS_FIELD)
                .is_some_and(|found| found.id() == arguments.id())
        })
        .and_then(|call| call.child_by_field_name(FUNCTION_FIELD))
        .map(without_parentheses)
    else {
        return false;
    };
    let Some(object) = callee
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == IDENTIFIER)
    else {
        return false;
    };
    let pattern = match context.text(object) {
        OBJECT_GLOBAL => OBJECT_MUTATIONS.as_ref(),
        REFLECT_GLOBAL => REFLECT_MUTATIONS.as_ref(),
        _ => None,
    };
    let property = match callee.kind() {
        MEMBER => callee
            .child_by_field_name(PROPERTY_FIELD)
            .map(|property| context.text(property).to_owned()),
        SUBSCRIPT => callee
            .child_by_field_name(INDEX_FIELD)
            .and_then(|index| static_string(without_parentheses(index), context.source)),
        _ => None,
    };
    let declared_in_global = tree
        .find(tree.scope_of(object), context.text(object))
        .and_then(|id| tree.variable(id))
        .is_none_or(|variable| {
            tree.scope(variable.scope)
                .is_some_and(|scope| scope.kind == ScopeKind::Global)
        });
    pattern
        .zip(property)
        .is_some_and(|(pattern, property)| pattern.is_match(&property))
        && declared_in_global
}
