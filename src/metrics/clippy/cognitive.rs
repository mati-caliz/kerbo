use tree_sitter::Node;

use super::macros;
use crate::syntax::{children_with_fields, named_children};

const BASE_COMPLEXITY: usize = 1;
const DECISION: usize = 1;
const SINGLE_ARM: usize = 1;
const COUNTED_RETURN: usize = 1;
const RETURNS_PER_DISCOUNT_WITHOUT_RESULT: usize = 2;
const BODY_FIELD: &str = "body";
const RETURN_TYPE_FIELD: &str = "return_type";
const CONDITION_FIELD: &str = "condition";
const PATTERN_FIELD: &str = "pattern";
const FUNCTION_FIELD: &str = "function";
const FIELD_FIELD: &str = "field";
const VALUE_FIELD: &str = "value";
const CONSEQUENCE_FIELD: &str = "consequence";
const SKIPPED_FIELDS: &[&str] = &[
    "pattern",
    "type",
    "parameters",
    "return_type",
    "type_arguments",
    "trait",
];
const IF: &str = "if_expression";
const WHILE: &str = "while_expression";
const FOR: &str = "for_expression";
const MATCH: &str = "match_expression";
const MATCH_ARM: &str = "match_arm";
const MATCH_PATTERN: &str = "match_pattern";
const TRY: &str = "try_expression";
const AWAIT: &str = "await_expression";
const RETURN: &str = "return_expression";
const MACRO: &str = "macro_invocation";
const CALL: &str = "call_expression";
const BLOCK: &str = "block";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const ITEM_SUFFIX: &str = "_item";
const ITEMS: &[&str] = &[
    "use_declaration",
    "macro_definition",
    "extern_crate_declaration",
    "associated_type",
];
const SEPARATE_BODIES: &[&str] = &["closure_expression", "async_block", "gen_block", "const_block"];
const BLOCKS_WITHOUT_EXPRESSION: &[&str] = &["loop_expression", "let_declaration", "unsafe_block"];
const EXPRESSIONS: &[&str] = &[
    "array_expression",
    "assignment_expression",
    "binary_expression",
    "boolean_literal",
    "break_expression",
    "call_expression",
    "char_literal",
    "compound_assignment_expr",
    "continue_expression",
    "field_expression",
    "float_literal",
    "generic_function",
    "identifier",
    "index_expression",
    "integer_literal",
    "let_condition",
    "loop_expression",
    "metavariable",
    "range_expression",
    "raw_string_literal",
    "reference_expression",
    "scoped_identifier",
    "self",
    "string_literal",
    "struct_expression",
    "try_block",
    "tuple_expression",
    "type_cast_expression",
    "unary_expression",
    "unit_expression",
    "unsafe_block",
    "yield_expression",
];
const RESULT_TYPE: &str = "Result";
const RESULT_TAILS: &[&str] = &["Ok", "Err", "ok_or", "ok_or_else", "map_err"];
const STATEMENTS: &[&str] = &["expression_statement", "let_declaration", "empty_statement"];
const PATH_SEPARATOR: &str = "::";
const GENERICS_START: char = '<';

#[derive(Debug)]
struct Tally {
    complexity: usize,
    returns: usize,
    after_return: bool,
}

impl Tally {
    const fn add(&mut self, amount: usize) {
        self.complexity = self.complexity.saturating_add(amount);
    }

    const fn expression(&mut self) {
        self.after_return = false;
    }

    const fn returned(&mut self) {
        if !self.after_return {
            self.returns = self.returns.saturating_add(COUNTED_RETURN);
        }
        self.after_return = true;
    }
}

#[derive(Debug, Clone, Copy)]
enum Step<'tree> {
    Enter(Node<'tree>),
    Expression,
    Returned,
}

// Calca cognitive_complexity de clippy, que no es la de Sonar: arranca en 1 y suma cada `if`,
// cada `match` de más de un brazo y cada guarda, sobre el código ya bajado a HIR. Por eso un
// `while` o un `for` suman uno (son un `if` y un `match` adentro de un `loop`), igual que cada `?`
// y cada `.await`. Después descuenta los `return`: todos si la función devuelve `Result`, la
// mitad si no, y un `return` justo después de otro no cuenta. Los cuerpos de los closures quedan
// afuera: cada closure se mide aparte. El salteo de `#[test]` que trae clippy no actúa, porque el
// atributo ya se expandió cuando el lint lo busca.
pub(super) fn of_function(function: Node<'_>, source: &[u8]) -> Option<usize> {
    let body = function.child_by_field_name(BODY_FIELD)?;
    let returns_result = function
        .child_by_field_name(RETURN_TYPE_FIELD)
        .is_some_and(|declared| names_result(declared, source));
    Some(score(body, source, returns_result))
}

pub(super) fn of_closure(closure: Node<'_>, source: &[u8]) -> Option<usize> {
    let body = closure.child_by_field_name(BODY_FIELD)?;
    let returns_result = closure.child_by_field_name(RETURN_TYPE_FIELD).map_or_else(
        || ends_in_result(body, source),
        |declared| names_result(declared, source),
    );
    Some(score(body, source, returns_result))
}

fn score(body: Node<'_>, source: &[u8], returns_result: bool) -> usize {
    let mut tally = Tally {
        complexity: BASE_COMPLEXITY,
        returns: 0,
        after_return: false,
    };
    visit(body, source, &mut tally);
    let adjustment = if returns_result {
        tally.returns
    } else {
        tally.returns / RETURNS_PER_DISCOUNT_WITHOUT_RESULT
    };
    tally
        .complexity
        .checked_sub(adjustment)
        .unwrap_or(tally.complexity)
}

fn visit(root: Node<'_>, source: &[u8], tally: &mut Tally) {
    let mut steps = vec![Step::Enter(root)];
    while let Some(step) = steps.pop() {
        match step {
            Step::Expression => tally.expression(),
            Step::Returned => tally.returned(),
            Step::Enter(node) => enter(node, source, tally, &mut steps),
        }
    }
}

fn enter<'tree>(node: Node<'tree>, source: &[u8], tally: &mut Tally, steps: &mut Vec<Step<'tree>>) {
    let kind = node.kind();
    if is_item(kind) {
        return;
    }
    if SEPARATE_BODIES.contains(&kind) {
        tally.expression();
        return;
    }
    if kind == MACRO {
        expand_macro(node, source, tally);
        return;
    }
    steps.extend(after_children(kind).iter().copied());
    match kind {
        IF | WHILE | FOR | AWAIT | TRY => {
            tally.add(DECISION);
            tally.expression();
        }
        MATCH => {
            tally.add(match_increment(node));
            tally.expression();
        }
        RETURN => tally.returned(),
        BLOCK => {
            if node
                .parent()
                .is_none_or(|parent| !BLOCKS_WITHOUT_EXPRESSION.contains(&parent.kind()))
            {
                tally.expression();
            }
        }
        _ if EXPRESSIONS.contains(&kind) => tally.expression(),
        _ => {}
    }
    steps.extend(visited_children(node).into_iter().rev().map(Step::Enter));
}

// Lo que rustc agrega después del código del usuario al bajar cada construcción: el `else { break }`
// del `while`, y el brazo con `return` que el `?` pone antes del que sigue de largo.
fn after_children(kind: &str) -> &'static [Step<'static>] {
    match kind {
        WHILE => &[Step::Expression, Step::Expression],
        TRY => &[Step::Expression, Step::Returned],
        AWAIT => &[Step::Expression],
        _ => &[],
    }
}

fn visited_children(node: Node<'_>) -> Vec<Node<'_>> {
    children_with_fields(node)
        .into_iter()
        .filter_map(|(field, child)| match field {
            Some(field) if SKIPPED_FIELDS.contains(&field) => Some(child)
                .filter(|pattern| pattern.kind() == MATCH_PATTERN)
                .and_then(|pattern| pattern.child_by_field_name(CONDITION_FIELD)),
            _ => Some(child),
        })
        .collect()
}

fn expand_macro(invocation: Node<'_>, source: &[u8], tally: &mut Tally) {
    tally.expression();
    let expansion = macros::expand(invocation, source);
    tally.add(expansion.added);
    for _ in 0..expansion.returns {
        tally.returned();
        tally.expression();
    }
    for argument in &expansion.arguments {
        if let Some(expression) = argument.expression() {
            visit(expression, argument.text.as_bytes(), tally);
        }
    }
    tally.expression();
}

fn match_increment(node: Node<'_>) -> usize {
    let arms: Vec<Node<'_>> = node
        .child_by_field_name(BODY_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|arm| arm.kind() == MATCH_ARM)
        .collect();
    let guards = arms
        .iter()
        .filter(|arm| {
            arm.child_by_field_name(PATTERN_FIELD)
                .and_then(|pattern| pattern.child_by_field_name(CONDITION_FIELD))
                .is_some()
        })
        .count();
    usize::from(arms.len() > SINGLE_ARM).saturating_add(guards)
}

fn is_item(kind: &str) -> bool {
    kind.ends_with(ITEM_SUFFIX) || ITEMS.contains(&kind)
}

// clippy mira el tipo resuelto, así que un alias como `io::Result` o `ClapResult<T>` también es
// `Result`. Sin tipos, cuenta el nombre exacto o uno terminado en `Result` con genéricos, que es
// la forma de un alias; un `SearchResult` sin genéricos suele ser un struct.
fn names_result(declared: Node<'_>, source: &[u8]) -> bool {
    let Some(text) = declared.utf8_text(source).ok() else {
        return false;
    };
    let mut parts = text.splitn(2, GENERICS_START);
    let name = parts
        .next()
        .and_then(|path| path.trim().rsplit(PATH_SEPARATOR).next())
        .unwrap_or_default();
    name == RESULT_TYPE || (name.ends_with(RESULT_TYPE) && parts.next().is_some())
}

// Sin tipos, lo que devuelve un closure se deduce de cómo termina: `Ok(..)`, `.ok_or(..)` o
// `.map_err(..)` es un `Result`. Sin esa pista no se toma como `Result`: un closure de
// `and_then` o de `filter_map` devuelve un `Option` aunque termine en cualquier otra llamada.
fn ends_in_result(expression: Node<'_>, source: &[u8]) -> bool {
    match expression.kind() {
        BLOCK => named_children(expression)
            .into_iter()
            .rev()
            .find(|child| !COMMENTS.contains(&child.kind()))
            .is_some_and(|last| {
                !STATEMENTS.contains(&last.kind()) && !is_item(last.kind()) && ends_in_result(last, source)
            }),
        IF => expression
            .child_by_field_name(CONSEQUENCE_FIELD)
            .is_some_and(|consequence| ends_in_result(consequence, source)),
        MATCH => expression
            .child_by_field_name(BODY_FIELD)
            .and_then(|arms| {
                named_children(arms)
                    .into_iter()
                    .find(|arm| arm.kind() == MATCH_ARM)
            })
            .and_then(|arm| arm.child_by_field_name(VALUE_FIELD))
            .is_some_and(|value| ends_in_result(value, source)),
        CALL => expression
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|callee| {
                let name = called_name(callee, source);
                RESULT_TAILS.contains(&name.rsplit(PATH_SEPARATOR).next().unwrap_or_default())
            }),
        _ => false,
    }
}

fn called_name<'source>(callee: Node<'_>, source: &'source [u8]) -> &'source str {
    callee
        .child_by_field_name(FIELD_FIELD)
        .unwrap_or(callee)
        .utf8_text(source)
        .unwrap_or_default()
}
