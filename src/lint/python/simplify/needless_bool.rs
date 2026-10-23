use tree_sitter::Node;

use crate::lint::python::Report;
use crate::lint::python::ast::{COMMENT, FALSE, TRUE, named, unparenthesized};
use crate::rules::PythonRule;

const RETURN: &str = "return_statement";
const ELSE: &str = "else_clause";
const ELIF: &str = "elif_clause";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const BODY_FIELD: &str = "body";

// La rama que devuelve un booleano y la que devuelve el otro, con dónde empieza el aviso.
struct Branches<'tree> {
    start: Node<'tree>,
    returned: Vec<Node<'tree>>,
    otherwise: Vec<Node<'tree>>,
}

// SIM103: `if cond: return True` seguido de `return False` (en un `else`, en el último `elif` con su
// `else`, o como la sentencia siguiente), que se escribe `return cond`. `is_excluded` dice si la
// condición del `if` es un chequeo de `sys.version_info`/`sys.platform` o `TYPE_CHECKING`.
pub fn needless_bool(statement: Node<'_>, is_excluded: &dyn Fn(Node<'_>) -> bool) -> Option<Report> {
    let branches = branches(statement)?;
    let returned = returned_bool(&branches.returned)?;
    let otherwise = returned_bool(&branches.otherwise)?;
    let condition = statement.child_by_field_name(CONDITION_FIELD)?;
    (returned != otherwise && !is_excluded(condition))
        .then(|| Report::at(PythonRule::SIM103, branches.start, ""))
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
                returned,
                otherwise: vec![next],
            })
        }
        [else_clause] if else_clause.kind() == ELSE => Some(Branches {
            start: statement,
            returned: statements(statement.child_by_field_name(CONSEQUENCE_FIELD)),
            otherwise: statements(else_clause.child_by_field_name(BODY_FIELD)),
        }),
        [.., elif_clause, else_clause] if elif_clause.kind() == ELIF && else_clause.kind() == ELSE => {
            Some(Branches {
                start: *elif_clause,
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
