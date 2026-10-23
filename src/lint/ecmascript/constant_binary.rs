use tree_sitter::Node;

use super::constants::{
    ARRAY, BOOLEAN, Constants, FALSE, FUNCTIONS, LITERALS, NEW, NOT, NULL, NULLISH, OBJECTS, REGEX, SEQUENCE,
    SPREAD, TRUE, TYPEOF, UNARY, UNDEFINED, VOID, last_expression, side, values,
};
use super::nodes::{
    ASSIGNMENT, AUGMENTED_ASSIGNMENT, BINARY, CALL, IDENTIFIER, LEFT_FIELD, RIGHT_FIELD, TEMPLATE_STRING,
    without_parentheses,
};
use super::strings::has_substitutions;
use super::tokens::{AND, OR};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const UPDATE: &str = "update_expression";
const ARGUMENT_FIELD: &str = "argument";
const TERNARY: &str = "ternary_expression";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const CONSTRUCTOR_FIELD: &str = "constructor";
const LOOSE_EQUALITY: &[&str] = &["==", "!="];
const STRICT_EQUALITY: &[&str] = &["===", "!=="];
const LOGICAL_ASSIGNMENTS: &[&str] = &["||=", "&&=", "??="];
const DELETE: &str = "delete";
const NEVER_NULLISH_CALLS: &[&str] = &["Boolean", "String", "Number", "Symbol", "BigInt"];
const NEVER_BOOLEAN_CALLS: &[&str] = &["String", "Number", "BigInt", "Symbol"];
const NUMERIC_OR_STRING_OPERATORS: &[&str] =
    &["+", "-", "*", "/", "%", "|", "^", "&", "**", "<<", ">>", ">>>"];
const ECMASCRIPT_GLOBALS: &[&str] = &[
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

// `no-constant-binary-expression` con las opciones de fábrica (sin comparaciones relacionales).
pub fn constant_binary_expressions(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    context
        .of_kind(&[BINARY])
        .filter_map(|node| constant_operand(&constants, node))
        .map(|operand| report(EslintRule::NoConstantBinaryExpression, operand, ""))
        .collect()
}

fn constant_operand<'tree>(constants: &Constants<'_, 'tree, '_>, node: Node<'tree>) -> Option<Node<'tree>> {
    let left = side(node, LEFT_FIELD)?;
    let right = side(node, RIGHT_FIELD)?;
    let operator = constants.operator(node);
    if constants.is_logical(node) {
        let constant = match operator {
            AND | OR => constants.is_constant(left, true),
            _ => operator == NULLISH && has_constant_nullishness(constants, left, false),
        };
        return constant.then_some(left);
    }
    if is_compared_constantly(constants, left, right, operator) {
        return Some(right);
    }
    if is_compared_constantly(constants, right, left, operator) {
        return Some(left);
    }
    if STRICT_EQUALITY.contains(&operator) {
        return [left, right]
            .into_iter()
            .find(|side| is_always_new(constants, *side));
    }
    (LOOSE_EQUALITY.contains(&operator) && is_always_new(constants, left) && is_always_new(constants, right))
        .then_some(left)
}

// `findBinaryExpressionConstantOperand`: `b` decide el resultado comparado contra `a`.
fn is_compared_constantly<'tree>(
    constants: &Constants<'_, 'tree, '_>,
    a: Node<'tree>,
    b: Node<'tree>,
    operator: &str,
) -> bool {
    let strict = STRICT_EQUALITY.contains(&operator);
    if !strict && !LOOSE_EQUALITY.contains(&operator) {
        return false;
    }
    (is_null_or_undefined(constants, a) && has_constant_nullishness(constants, b, false))
        || (is_static_boolean(constants, a)
            && if strict {
                has_constant_strict_boolean_comparison(constants, b)
            } else {
                has_constant_loose_boolean_comparison(constants, b)
            })
}

fn is_null_or_undefined(constants: &Constants<'_, '_, '_>, node: Node<'_>) -> bool {
    node.kind() == NULL
        || constants.is_global_undefined(node)
        || (node.kind() == UNARY && constants.operator(node) == VOID)
}

fn has_constant_nullishness<'tree>(
    constants: &Constants<'_, 'tree, '_>,
    node: Node<'tree>,
    non_nullish: bool,
) -> bool {
    let node = without_parentheses(node);
    if non_nullish && is_null_or_undefined(constants, node) {
        return false;
    }
    if constants.is_logical(node) {
        return constants.operator(node) == NULLISH
            && side(node, RIGHT_FIELD).is_some_and(|right| has_constant_nullishness(constants, right, true));
    }
    let kind = node.kind();
    if is_object_like(kind)
        || LITERALS.contains(&kind)
        || [NEW, TEMPLATE_STRING, UPDATE, BINARY, UNARY].contains(&kind)
    {
        return true;
    }
    match kind {
        IDENTIFIER | UNDEFINED => constants.is_global_undefined(node),
        CALL => constants
            .called_global(node)
            .is_some_and(|(name, _)| NEVER_NULLISH_CALLS.contains(&name)),
        ASSIGNMENT => side(node, RIGHT_FIELD)
            .is_some_and(|right| has_constant_nullishness(constants, right, non_nullish)),
        AUGMENTED_ASSIGNMENT => !LOGICAL_ASSIGNMENTS.contains(&constants.operator(node)),
        SEQUENCE => {
            last_expression(node).is_some_and(|last| has_constant_nullishness(constants, last, non_nullish))
        }
        _ => false,
    }
}

fn is_object_like(kind: &str) -> bool {
    kind == ARRAY || FUNCTIONS.contains(&kind) || OBJECTS.contains(&kind)
}

fn is_static_boolean<'tree>(constants: &Constants<'_, 'tree, '_>, node: Node<'tree>) -> bool {
    match node.kind() {
        TRUE | FALSE => true,
        CALL => constants.is_boolean_call(node),
        UNARY => {
            constants.operator(node) == NOT
                && side(node, ARGUMENT_FIELD).is_some_and(|argument| constants.is_constant(argument, true))
        }
        _ => false,
    }
}

fn has_constant_loose_boolean_comparison<'tree>(
    constants: &Constants<'_, 'tree, '_>,
    node: Node<'tree>,
) -> bool {
    let node = without_parentheses(node);
    let kind = node.kind();
    if OBJECTS.contains(&kind) || FUNCTIONS.contains(&kind) || LITERALS.contains(&kind) {
        return true;
    }
    match kind {
        ARRAY => {
            let elements = values(node);
            let has_holes = node.children(&mut node.walk()).any(|child| child.kind() == ",");
            (elements.is_empty() && !has_holes)
                || elements.iter().filter(|element| element.kind() != SPREAD).count() > 1
        }
        UNARY => match constants.operator(node) {
            VOID | TYPEOF => true,
            NOT => side(node, ARGUMENT_FIELD).is_some_and(|argument| constants.is_constant(argument, true)),
            _ => false,
        },
        CALL => constants.is_boolean_call(node),
        IDENTIFIER | UNDEFINED => constants.is_global_undefined(node),
        TEMPLATE_STRING => !has_substitutions(node),
        ASSIGNMENT => side(node, RIGHT_FIELD)
            .is_some_and(|right| has_constant_loose_boolean_comparison(constants, right)),
        SEQUENCE => {
            last_expression(node).is_some_and(|last| has_constant_loose_boolean_comparison(constants, last))
        }
        _ => false,
    }
}

fn has_constant_strict_boolean_comparison<'tree>(
    constants: &Constants<'_, 'tree, '_>,
    node: Node<'tree>,
) -> bool {
    let node = without_parentheses(node);
    let kind = node.kind();
    if is_object_like(kind) || LITERALS.contains(&kind) || [NEW, TEMPLATE_STRING, UPDATE].contains(&kind) {
        return true;
    }
    if constants.is_binary(node) {
        return NUMERIC_OR_STRING_OPERATORS.contains(&constants.operator(node));
    }
    match kind {
        UNARY => match constants.operator(node) {
            DELETE => false,
            NOT => side(node, ARGUMENT_FIELD).is_some_and(|argument| constants.is_constant(argument, true)),
            _ => true,
        },
        SEQUENCE => {
            last_expression(node).is_some_and(|last| has_constant_strict_boolean_comparison(constants, last))
        }
        IDENTIFIER | UNDEFINED => constants.is_global_undefined(node),
        ASSIGNMENT => side(node, RIGHT_FIELD)
            .is_some_and(|right| has_constant_strict_boolean_comparison(constants, right)),
        AUGMENTED_ASSIGNMENT => !LOGICAL_ASSIGNMENTS.contains(&constants.operator(node)),
        CALL => constants.called_global(node).is_some_and(|(name, arguments)| {
            NEVER_BOOLEAN_CALLS.contains(&name)
                || (name == BOOLEAN
                    && arguments
                        .first()
                        .is_none_or(|first| constants.is_constant(*first, true)))
        }),
        _ => false,
    }
}

fn is_always_new<'tree>(constants: &Constants<'_, 'tree, '_>, node: Node<'tree>) -> bool {
    let node = without_parentheses(node);
    let kind = node.kind();
    if is_object_like(kind) {
        return true;
    }
    match kind {
        NEW => node
            .child_by_field_name(CONSTRUCTOR_FIELD)
            .map(without_parentheses)
            .filter(|constructor| constructor.kind() == IDENTIFIER)
            .is_some_and(|constructor| {
                ECMASCRIPT_GLOBALS.contains(&constants.text(constructor)) && constants.is_global(constructor)
            }),
        REGEX => true,
        SEQUENCE => last_expression(node).is_some_and(|last| is_always_new(constants, last)),
        ASSIGNMENT => side(node, RIGHT_FIELD).is_some_and(|right| is_always_new(constants, right)),
        TERNARY => [CONSEQUENCE_FIELD, ALTERNATIVE_FIELD]
            .iter()
            .all(|field| side(node, field).is_some_and(|branch| is_always_new(constants, branch))),
        _ => false,
    }
}
