use tree_sitter::Node;

use crate::fix::{Fix, TextEdit};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    COMMENT, COMPARISON, FALSE, NOT_OPERATOR, OPERATORS_FIELD, TRUE, named, text, unparenthesized,
};
use crate::lint::python::codegen;
use crate::lint::python::fix_edits::{has_comment_within, point_of};
use crate::rules::PythonRule;

const RETURN: &str = "return_statement";
const ELSE: &str = "else_clause";
const ELIF: &str = "elif_clause";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const BODY_FIELD: &str = "body";
const ARGUMENT_FIELD: &str = "argument";
const BOOL: &str = "bool";
const NEGATIONS: [(&str, &str); 6] = [
    ("==", "!="),
    ("!=", "=="),
    ("in", "not in"),
    ("not in", "in"),
    ("is", "is not"),
    ("is not", "is"),
];

// La rama que devuelve un booleano y la que devuelve el otro, con dónde empieza el aviso.
struct Branches<'tree> {
    start: Node<'tree>,
    end: Node<'tree>,
    test: Option<Node<'tree>>,
    returned: Vec<Node<'tree>>,
    otherwise: Vec<Node<'tree>>,
}

// SIM103: `if cond: return True` seguido de `return False` (en un `else`, en el último `elif` con su
// `else`, o como la sentencia siguiente), que se escribe `return cond`. `is_excluded` dice si la
// condición del `if` es un chequeo de `sys.version_info`/`sys.platform` o `TYPE_CHECKING`.
pub fn needless_bool(
    statement: Node<'_>,
    is_excluded: &dyn Fn(Node<'_>) -> bool,
    source: &[u8],
) -> Option<Report> {
    let branches = branches(statement)?;
    let returned = returned_bool(&branches.returned)?;
    let otherwise = returned_bool(&branches.otherwise)?;
    let condition = statement.child_by_field_name(CONDITION_FIELD)?;
    (returned != otherwise && !is_excluded(condition)).then(|| {
        let fix = branches
            .test
            .and_then(|test| returned_condition_fix(&branches, test, !returned, source));
        Report::at(PythonRule::SIM103, branches.start, "").with_fix(fix)
    })
}

// El arreglo de SIM103: `return cond` (o la condición negada, o `bool(cond)` si no es una
// comparación) en lugar de todo el bloque; inseguro, y sin arreglo si hay comentarios.
fn returned_condition_fix(
    branches: &Branches<'_>,
    test: Node<'_>,
    inverted: bool,
    source: &[u8],
) -> Option<Fix> {
    let (start, end) = (branches.start.start_byte(), branches.end.end_byte());
    if has_comment_within(branches.start.parent().unwrap_or(branches.start), start, end) {
        return None;
    }
    let test = unparenthesized(test);
    let condition = if inverted {
        negated(test, source)
    } else if test.kind() == COMPARISON {
        codegen::unparse(test, 0, source)
    } else {
        format!("{BOOL}({})", codegen::unparse(test, codegen::COMMA, source))
    };
    let edit = TextEdit {
        start: point_of(source, start),
        end: point_of(source, end),
        replacement: format!("return {condition}"),
    };
    Some(Fix::from_edits(source, vec![edit]).marked_unsafe())
}

fn negated(test: Node<'_>, source: &[u8]) -> String {
    if test.kind() == NOT_OPERATOR
        && let Some(operand) = test.child_by_field_name(ARGUMENT_FIELD)
    {
        return codegen::unparse(operand, 0, source);
    }
    if test.kind() == COMPARISON {
        let operands = named(test);
        let mut cursor = test.walk();
        let operators: Vec<String> = test
            .children_by_field_name(OPERATORS_FIELD, &mut cursor)
            .map(|operator| {
                text(operator, source)
                    .split_whitespace()
                    .collect::<Vec<&str>>()
                    .join(" ")
            })
            .collect();
        if let ([left, right], [operator]) = (operands.as_slice(), operators.as_slice())
            && let Some((_, opposite)) = NEGATIONS.iter().find(|(original, _)| original == operator)
        {
            let level = codegen::CMP.saturating_add(1);
            return format!(
                "{} {opposite} {}",
                codegen::unparse(*left, level, source),
                codegen::unparse(*right, level, source)
            );
        }
    }
    format!("not {}", codegen::unparse(test, codegen::NOT, source))
}

fn branches(statement: Node<'_>) -> Option<Branches<'_>> {
    let clauses: Vec<Node<'_>> = {
        let mut cursor = statement.walk();
        statement
            .children_by_field_name(ALTERNATIVE_FIELD, &mut cursor)
            .collect()
    };
    match clauses.as_slice() {
        [] => {
            let returned = statements(statement.child_by_field_name(CONSEQUENCE_FIELD));
            returned_bool(&returned)?;
            let next = next_statement(statement).filter(|next| next.kind() == RETURN)?;
            Some(Branches {
                start: statement,
                end: next,
                test: statement.child_by_field_name(CONDITION_FIELD),
                returned,
                otherwise: vec![next],
            })
        }
        [else_clause] if else_clause.kind() == ELSE => Some(Branches {
            start: statement,
            end: statement,
            test: statement.child_by_field_name(CONDITION_FIELD),
            returned: statements(statement.child_by_field_name(CONSEQUENCE_FIELD)),
            otherwise: statements(else_clause.child_by_field_name(BODY_FIELD)),
        }),
        [.., elif_clause, else_clause] if elif_clause.kind() == ELIF && else_clause.kind() == ELSE => {
            Some(Branches {
                start: *elif_clause,
                end: *else_clause,
                test: elif_clause.child_by_field_name(CONDITION_FIELD),
                returned: statements(elif_clause.child_by_field_name(CONSEQUENCE_FIELD)),
                otherwise: statements(else_clause.child_by_field_name(BODY_FIELD)),
            })
        }
        _ => None,
    }
}

fn statements(block: Option<Node<'_>>) -> Vec<Node<'_>> {
    block.map_or_else(Vec::new, |block| {
        named(block)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
            .collect()
    })
}

fn next_statement(statement: Node<'_>) -> Option<Node<'_>> {
    let mut next = statement.next_named_sibling();
    while let Some(candidate) = next.filter(|candidate| candidate.kind() == COMMENT) {
        next = candidate.next_named_sibling();
    }
    next
}

// `is_one_line_return_bool`: un único `return True` o `return False`.
fn returned_bool(body: &[Node<'_>]) -> Option<bool> {
    let [statement] = body else {
        return None;
    };
    if statement.kind() != RETURN {
        return None;
    }
    let values: Vec<Node<'_>> = named(*statement)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    match values.as_slice() {
        [value] => match unparenthesized(*value).kind() {
            TRUE => Some(true),
            FALSE => Some(false),
            _ => None,
        },
        _ => None,
    }
}
