use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::nodes::{
    FUNCTION_FIELD, NAME_FIELD, OBJECT_FIELD, STRING, VALUE_FIELD, text, without_parentheses,
};
use super::strings::{cooked, numeric_text};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const ENUM_ASSIGNMENT: &str = "enum_assignment";
const ENUM_BODY: &str = "enum_body";
const NUMBER: &str = "number";
const TEMPLATE: &str = "template_string";
const SUBSTITUTION: &str = "template_substitution";
const UNARY: &str = "unary_expression";
const ARGUMENT_FIELD: &str = "argument";
const OPERATOR_FIELD: &str = "operator";
const MINUS: &str = "-";
const PLUS: &str = "+";
const NON_NULL: &str = "non_null_expression";
const MEMBER: &str = "member_expression";
const CALL: &str = "call_expression";
const OPTIONAL_CHAIN_FIELD: &str = "optional_chain";
const OPTIONAL_CALL: &str = "?.";
const INTERFACE: &str = "interface_declaration";
const CLASS: &str = "class_declaration";
const ABSTRACT_CLASS: &str = "abstract_class_declaration";
const CLASS_EXPRESSION: &str = "class";
const CONSTRUCT_SIGNATURE: &str = "construct_signature";
const METHOD_SIGNATURE: &str = "method_signature";
const INTERFACE_BODY: &str = "interface_body";
const CLASS_BODY: &str = "class_body";
const TYPE_FIELD: &str = "type";
const RETURN_TYPE_FIELD: &str = "return_type";
const TYPE_IDENTIFIER: &str = "type_identifier";
const CONSTRUCTOR_NAME: &str = "constructor";
const NEW_NAME: &str = "new";
const TYPE_PARAMETER: &str = "type_parameter";
const CONSTRAINT_FIELD: &str = "constraint";
const ANY_TYPES: &[&str] = &["any", "unknown"];
const AS_EXPRESSION: &str = "as_expression";
const TYPE_ASSERTION: &str = "type_assertion";
const LITERAL_TYPE: &str = "literal_type";
const DECLARATOR: &str = "variable_declarator";
const FIELD: &str = "public_field_definition";
const COMPARABLE_LITERALS: &[&str] = &[STRING, NUMBER, "true", "false"];
const MODULE: &str = "module";
const AMBIENT: &str = "ambient_declaration";
const SCOPE_BLOCKS: &[&str] = &["program", "statement_block"];

fn named(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

#[derive(PartialEq)]
enum EnumValue {
    Text(String),
    Number(f64),
}

fn enum_value(node: Node<'_>, source: &[u8]) -> Option<EnumValue> {
    let raw = text(node, source);
    match node.kind() {
        STRING => Some(EnumValue::Text(cooked(raw.get(1..raw.len().saturating_sub(1))?))),
        NUMBER => numeric_text(raw)?.parse().ok().map(EnumValue::Number),
        TEMPLATE if !named(node).iter().any(|part| part.kind() == SUBSTITUTION) => {
            Some(EnumValue::Text(cooked(raw.get(1..raw.len().saturating_sub(1))?)))
        }
        UNARY => {
            let inner = match enum_value(node.child_by_field_name(ARGUMENT_FIELD)?, source)? {
                EnumValue::Number(number) => number,
                EnumValue::Text(value) => value.trim().parse().ok()?,
            };
            match text(node.child_by_field_name(OPERATOR_FIELD)?, source) {
                MINUS => Some(EnumValue::Number(-inner)),
                PLUS => Some(EnumValue::Number(inner)),
                _ => None,
            }
        }
        _ => None,
    }
}

// `no-duplicate-enum-values`.
pub fn duplicate_enum_values(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[ENUM_BODY])
        .flat_map(|body| {
            let mut seen: Vec<EnumValue> = Vec::new();
            let mut found = Vec::new();
            for member in named(body)
                .into_iter()
                .filter(|member| member.kind() == ENUM_ASSIGNMENT)
            {
                let Some(value) = member
                    .child_by_field_name(VALUE_FIELD)
                    .and_then(|value| enum_value(value, context.source))
                else {
                    continue;
                };
                if seen.contains(&value) {
                    found.push(report(EslintRule::NoDuplicateEnumValues, member, ""));
                } else {
                    seen.push(value);
                }
            }
            found
        })
        .collect()
}

// `no-extra-non-null-assertion`: `a!!`, `a!?.b` y `a!?.()`.
pub fn extra_non_null_assertions(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[NON_NULL])
        .filter(|assertion| is_extra_assertion(*assertion))
        .map(|assertion| report(EslintRule::NoExtraNonNullAssertion, assertion, ""))
        .collect()
}

fn is_extra_assertion(assertion: Node<'_>) -> bool {
    let mut child = assertion;
    let mut parent = assertion.parent();
    while let Some(holder) = parent.filter(|holder| holder.kind() == "parenthesized_expression") {
        child = holder;
        parent = holder.parent();
    }
    let Some(holder) = parent else {
        return false;
    };
    match holder.kind() {
        NON_NULL => true,
        MEMBER => {
            holder.child_by_field_name(OBJECT_FIELD) == Some(child)
                && holder.child_by_field_name(OPTIONAL_CHAIN_FIELD).is_some()
        }
        CALL => {
            holder.child_by_field_name(FUNCTION_FIELD) == Some(child)
                && (holder.child_by_field_name(OPTIONAL_CHAIN_FIELD).is_some()
                    || has_optional_chain_token(child))
        }
        _ => false,
    }
}

fn has_optional_chain_token(callee: Node<'_>) -> bool {
    callee
        .next_sibling()
        .is_some_and(|next| next.kind() == OPTIONAL_CALL)
}

// `no-misused-new`.
pub fn misused_new(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let source = context.source;
    context
        .of_kind(&[CONSTRUCT_SIGNATURE, METHOD_SIGNATURE])
        .filter(|signature| {
            let holder = signature.parent();
            let owner = holder.and_then(|holder| holder.parent());
            let name = signature
                .child_by_field_name(NAME_FIELD)
                .map_or("", |name| text(name, source));
            if signature.kind() == CONSTRUCT_SIGNATURE {
                return holder.is_some_and(|holder| holder.kind() == INTERFACE_BODY)
                    && returns_owner(*signature, owner, TYPE_FIELD, source);
            }
            match holder.map(|holder| holder.kind()) {
                Some(INTERFACE_BODY | "object_type") if name == CONSTRUCTOR_NAME => true,
                Some(CLASS_BODY) if name == NEW_NAME => {
                    returns_owner(*signature, owner, RETURN_TYPE_FIELD, source)
                }
                _ => false,
            }
        })
        .map(|signature| report(EslintRule::NoMisusedNew, signature, ""))
        .collect()
}

fn returns_owner(signature: Node<'_>, owner: Option<Node<'_>>, field: &str, source: &[u8]) -> bool {
    let owner_name = owner
        .filter(|owner| [INTERFACE, CLASS, ABSTRACT_CLASS, CLASS_EXPRESSION].contains(&owner.kind()))
        .and_then(|owner| owner.child_by_field_name(NAME_FIELD))
        .map(|name| text(name, source));
    let returned = signature
        .child_by_field_name(field)
        .and_then(|annotation| annotation.named_child(0))
        .filter(|returned| returned.kind() == TYPE_IDENTIFIER)
        .map(|returned| text(returned, source));
    owner_name.is_some() && owner_name == returned
}

// `no-unnecessary-type-constraint`: `<T extends any>` y `<T extends unknown>`.
pub fn unnecessary_type_constraints(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let source = context.source;
    context
        .of_kind(&[TYPE_PARAMETER])
        .filter(|parameter| {
            parameter
                .child_by_field_name(CONSTRAINT_FIELD)
                .and_then(|constraint| constraint.named_child(0))
                .is_some_and(|constraint| ANY_TYPES.contains(&text(constraint, source)))
        })
        .map(|parameter| report(EslintRule::NoUnnecessaryTypeConstraint, parameter, ""))
        .collect()
}

// `no-unsafe-declaration-merging`: una clase y una interfaz con el mismo nombre en el mismo alcance.
pub fn unsafe_declaration_merging(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let source = context.source;
    let mut classes: HashSet<(usize, &str)> = HashSet::new();
    let mut interfaces: HashSet<(usize, &str)> = HashSet::new();
    let mut declared: HashMap<(usize, &str), Vec<Node<'_>>> = HashMap::new();
    for node in context.of_kind(&[CLASS, ABSTRACT_CLASS, INTERFACE]) {
        let Some(name) = node.child_by_field_name(NAME_FIELD) else {
            continue;
        };
        let key = (enclosing_block(node), text(name, source));
        if node.kind() == INTERFACE {
            interfaces.insert(key);
        } else {
            classes.insert(key);
        }
        declared.entry(key).or_default().push(name);
    }
    let mut found: Vec<LintFinding> = declared
        .into_iter()
        .filter(|(key, _)| classes.contains(key) && interfaces.contains(key))
        .flat_map(|(_, names)| names)
        .map(|name| report(EslintRule::NoUnsafeDeclarationMerging, name, ""))
        .collect();
    found.sort_by_key(|finding| (finding.line, finding.column));
    found
}

fn enclosing_block(node: Node<'_>) -> usize {
    std::iter::successors(node.parent(), Node::parent)
        .find(|ancestor| SCOPE_BLOCKS.contains(&ancestor.kind()))
        .map_or(0, |block| block.id())
}

// `prefer-as-const`: `'a' as 'a'`, `<'a'>'a'` y `let x: 'a' = 'a'`.
pub fn literal_type_repetitions(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let source = context.source;
    context
        .of_kind(&[AS_EXPRESSION, TYPE_ASSERTION, DECLARATOR, FIELD])
        .filter_map(|node| {
            let (value, literal_type) = match node.kind() {
                AS_EXPRESSION => {
                    let parts = named(node);
                    (parts.first().copied()?, parts.get(1).copied()?)
                }
                TYPE_ASSERTION => {
                    let parts = named(node);
                    (parts.get(1).copied()?, parts.first().copied()?.named_child(0)?)
                }
                _ => (
                    node.child_by_field_name(VALUE_FIELD)?,
                    node.child_by_field_name(TYPE_FIELD)?.named_child(0)?,
                ),
            };
            let value = without_parentheses(value);
            let literal = (literal_type.kind() == LITERAL_TYPE).then(|| literal_type.named_child(0))??;
            (COMPARABLE_LITERALS.contains(&value.kind())
                && value.kind() == literal.kind()
                && text(value, source) == text(literal, source))
            .then(|| {
                report(
                    EslintRule::PreferAsConst,
                    literal_type,
                    text(literal_type, source),
                )
            })
        })
        .collect()
}

// `prefer-namespace-keyword`: `module A {}`. `declare module 'a'` y `declare global` siguen siendo válidos.
pub fn module_keywords(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[MODULE])
        .filter(|module| {
            module
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| name.kind() != STRING)
        })
        .map(|module| {
            let position = module
                .parent()
                .filter(|parent| parent.kind() == AMBIENT)
                .unwrap_or(module);
            report(EslintRule::PreferNamespaceKeyword, position, "")
        })
        .collect()
}
