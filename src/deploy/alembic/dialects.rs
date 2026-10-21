use tree_sitter::Node;

use crate::syntax::children_with_fields;

const POSTGRES: &str = "postgres";
const OTHER_DIALECTS: &[&str] = &["mysql", "sqlite", "mssql", "oracle"];
const NEGATIONS: &[&str] = &["!=", "not "];
const CONDITION: &str = "condition";
const CONSEQUENCE: &str = "consequence";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Postgres,
    Other,
    Unknown,
}

fn dialect_of(condition: Node<'_>, source: &[u8]) -> Dialect {
    let text = condition.utf8_text(source).unwrap_or_default().to_lowercase();
    let negated = NEGATIONS.iter().any(|negation| text.contains(negation));
    let postgres = text.contains(POSTGRES);
    let other = OTHER_DIALECTS.iter().any(|dialect| text.contains(dialect));
    match (negated, postgres, other) {
        (false, true, false) | (true, false, true) => Dialect::Postgres,
        (false, false, true) | (true, true, false) => Dialect::Other,
        _ => Dialect::Unknown,
    }
}

pub fn postgres_children<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    let children = children_with_fields(node);
    if node.kind() != "if_statement" && node.kind() != "elif_clause" {
        return children.into_iter().map(|(_, child)| child).collect();
    }
    let dialect = children
        .iter()
        .find(|(field, _)| *field == Some(CONDITION))
        .map_or(Dialect::Unknown, |(_, condition)| dialect_of(*condition, source));
    let branch_is_postgres = dialect == Dialect::Postgres
        || children
            .iter()
            .filter(|(_, child)| child.kind() == "elif_clause")
            .filter_map(|(_, clause)| clause.child_by_field_name(CONDITION))
            .any(|condition| dialect_of(condition, source) == Dialect::Postgres);
    children
        .into_iter()
        .filter(|(field, child)| {
            let skipped_consequence = *field == Some(CONSEQUENCE) && dialect == Dialect::Other;
            let skipped_else = child.kind() == "else_clause" && branch_is_postgres;
            !(skipped_consequence || skipped_else)
        })
        .map(|(_, child)| child)
        .collect()
}

pub fn returns_for_postgres(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "if_statement"
        && node
            .child_by_field_name(CONDITION)
            .is_some_and(|condition| dialect_of(condition, source) == Dialect::Postgres)
        && node.child_by_field_name(CONSEQUENCE).is_some_and(|block| {
            crate::syntax::named_children(block)
                .iter()
                .any(|statement| statement.kind() == "return_statement")
        })
}
