use tree_sitter::Node;

use super::go_kinds::{
    ASSIGN_STMT, BLOCK_STMT, BRANCH_STMT, CASE_CLAUSE, COMM_CLAUSE, DECL_STMT, DEFER_STMT, EMPTY_STMT,
    EXPR_STMT, FILE, FOR_STMT, FUNC_DECL, GEN_DECL, GO_STMT, IDENT, IF_STMT, INC_DEC_STMT, LABELED_STMT,
    RANGE_STMT, RETURN_STMT, SELECT_STMT, SEND_STMT, SWITCH_STMT, TYPE_ASSERT_EXPR, TYPE_SPEC,
    TYPE_SWITCH_STMT, VALUE_SPEC,
};
use super::go_syntax::{Builder, field, signature_span, significant_children, span, token, token_start};
use super::go_types::{field_list, func_type};

const SKIPPED_TOP_LEVEL: &[&str] = &["package_clause", "import_declaration"];
const DECLARATIONS: &[&str] = &["var_declaration", "const_declaration", "type_declaration"];
const BRANCHES: &[&str] = &[
    "break_statement",
    "continue_statement",
    "goto_statement",
    "fallthrough_statement",
];
const SINGLES: &[(&str, i32)] = &[
    ("expression_statement", EXPR_STMT),
    ("go_statement", GO_STMT),
    ("defer_statement", DEFER_STMT),
    ("inc_statement", INC_DEC_STMT),
    ("dec_statement", INC_DEC_STMT),
];
const SPEC_LIST: &str = "var_spec_list";
const STATEMENT_LIST: &str = "statement_list";
const RANGE_CLAUSE: &str = "range_clause";
const FOR_CLAUSE: &str = "for_clause";

#[derive(Debug, Clone, Copy)]
enum CaseLabels {
    Values,
    Types,
    Communication,
}

pub fn source_file(builder: &mut Builder<'_>, root: Node<'_>) {
    let children = significant_children(root);
    let start = children
        .first()
        .map_or_else(|| root.start_byte(), Node::start_byte);
    let declarations: Vec<Node<'_>> = children
        .into_iter()
        .filter(|child| !SKIPPED_TOP_LEVEL.contains(&child.kind()))
        .collect();
    let end = declarations
        .last()
        .map_or_else(|| root.end_byte(), Node::end_byte);
    builder.wrap(FILE, (start, end), |inner| {
        for declaration in declarations {
            top_level(inner, declaration);
        }
    });
}

fn top_level(builder: &mut Builder<'_>, node: Node<'_>) {
    match node.kind() {
        "function_declaration" | "method_declaration" => function_declaration(builder, node),
        kind if DECLARATIONS.contains(&kind) => general_declaration(builder, node),
        _ => statement(builder, node),
    }
}

fn function_declaration(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(FUNC_DECL, span(node), |inner| {
        if let Some(receiver) = field(node, "receiver") {
            field_list(inner, receiver);
        }
        inner.optional(field(node, "name"));
        func_type(
            inner,
            signature_span(node, node.start_byte()),
            field(node, "type_parameters"),
            node,
        );
        if let Some(body) = field(node, "body") {
            block(inner, body);
        }
    });
}

fn general_declaration(builder: &mut Builder<'_>, node: Node<'_>) {
    let specs: Vec<Node<'_>> = significant_children(node)
        .into_iter()
        .flat_map(|spec| {
            if spec.kind() == SPEC_LIST {
                significant_children(spec)
            } else {
                vec![spec]
            }
        })
        .collect();
    builder.wrap(GEN_DECL, span(node), |inner| {
        for spec in specs {
            specification(inner, spec);
        }
    });
}

fn specification(builder: &mut Builder<'_>, node: Node<'_>) {
    match node.kind() {
        "type_spec" | "type_alias" => builder.wrap(TYPE_SPEC, span(node), |inner| type_spec(inner, node)),
        _ => builder.wrap(VALUE_SPEC, span(node), |inner| value_spec(inner, node)),
    }
}

fn type_spec(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.optional(field(node, "name"));
    if let Some(parameters) = field(node, "type_parameters") {
        field_list(builder, parameters);
    }
    builder.optional(field(node, "type"));
}

fn value_spec(builder: &mut Builder<'_>, node: Node<'_>) {
    let mut cursor = node.walk();
    let names: Vec<Node<'_>> = node
        .children_by_field_name("name", &mut cursor)
        .filter(Node::is_named)
        .collect();
    for name in names {
        builder.leaf(IDENT, name);
    }
    builder.optional(field(node, "type"));
    builder.expressions(field(node, "value"));
}

pub fn block(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(BLOCK_STMT, span(node), |inner| statements(inner, node));
}

fn statements(builder: &mut Builder<'_>, container: Node<'_>) {
    for child in significant_children(container) {
        if child.kind() == STATEMENT_LIST {
            statements(builder, child);
        } else {
            statement(builder, child);
        }
    }
}

fn body_end(node: Node<'_>) -> usize {
    significant_children(node)
        .into_iter()
        .filter(|child| child.kind() == STATEMENT_LIST)
        .filter_map(|list| significant_children(list).last().map(Node::end_byte))
        .next_back()
        .or_else(|| token(node, ":").map(|colon| colon.end_byte()))
        .unwrap_or_else(|| node.end_byte())
}

fn statement(builder: &mut Builder<'_>, node: Node<'_>) {
    let kind = node.kind();
    if DECLARATIONS.contains(&kind) {
        return builder.wrap(DECL_STMT, span(node), |inner| general_declaration(inner, node));
    }
    if let Some(&(_, wrapped)) = SINGLES.iter().find(|(name, _)| *name == kind) {
        return builder.wrap(wrapped, span(node), |inner| inner.first_child(node));
    }
    if BRANCHES.contains(&kind) {
        return builder.wrap(BRANCH_STMT, span(node), |inner| branch_label(inner, node));
    }
    match kind {
        "send_statement" => builder.wrap(SEND_STMT, span(node), |inner| send(inner, node)),
        "assignment_statement" | "short_var_declaration" => assignment(builder, node),
        "receive_statement" if field(node, "left").is_some() => assignment(builder, node),
        "receive_statement" => builder.wrap(EXPR_STMT, span(node), |inner| {
            inner.optional(field(node, "right"));
        }),
        "return_statement" => builder.wrap(RETURN_STMT, span(node), |inner| {
            inner.expressions(significant_children(node).first().copied());
        }),
        "block" => block(builder, node),
        "empty_statement" => builder.leaf(EMPTY_STMT, node),
        "labeled_statement" => builder.wrap(LABELED_STMT, span(node), |inner| labeled(inner, node)),
        "if_statement" => builder.wrap(IF_STMT, span(node), |inner| if_parts(inner, node)),
        "for_statement" => for_statement(builder, node),
        "expression_switch_statement" => {
            builder.wrap(SWITCH_STMT, span(node), |inner| switch_parts(inner, node));
        }
        "type_switch_statement" => builder.wrap(TYPE_SWITCH_STMT, span(node), |inner| {
            type_switch_parts(inner, node);
        }),
        "select_statement" => builder.wrap(SELECT_STMT, span(node), |inner| {
            cases_block(inner, node, COMM_CLAUSE, CaseLabels::Communication);
        }),
        _ => builder.unknown(node),
    }
}

fn branch_label(builder: &mut Builder<'_>, node: Node<'_>) {
    for label in significant_children(node) {
        builder.leaf(IDENT, label);
    }
}

fn send(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.optional(field(node, "channel"));
    builder.optional(field(node, "value"));
}

fn assignment(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(ASSIGN_STMT, span(node), |inner| {
        let right = field(node, "right");
        if right.is_some_and(|value| value.kind() == "expression_list") {
            inner.expressions(right);
        } else {
            inner.optional(right);
        }
        inner.expressions(field(node, "left"));
    });
}

fn labeled(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.optional(field(node, "label"));
    let labeled_statement = significant_children(node)
        .into_iter()
        .find(|child| child.kind() != "label_name");
    match labeled_statement {
        Some(found) => statement(builder, found),
        None => builder.wrap(EMPTY_STMT, (node.end_byte(), node.end_byte()), |_| {}),
    }
}

fn initializer(builder: &mut Builder<'_>, node: Node<'_>, name: &str) {
    if let Some(found) = field(node, name) {
        statement(builder, found);
    }
}

fn if_parts(builder: &mut Builder<'_>, node: Node<'_>) {
    initializer(builder, node, "initializer");
    builder.optional(field(node, "condition"));
    if let Some(consequence) = field(node, "consequence") {
        block(builder, consequence);
    }
    initializer(builder, node, "alternative");
}

fn for_statement(builder: &mut Builder<'_>, node: Node<'_>) {
    let body = field(node, "body");
    let header = significant_children(node)
        .into_iter()
        .find(|child| body.is_none_or(|found| found.id() != child.id()));
    let is_range = header.is_some_and(|clause| clause.kind() == RANGE_CLAUSE);
    builder.wrap(
        if is_range { RANGE_STMT } else { FOR_STMT },
        span(node),
        |inner| {
            for_header(inner, header);
            if let Some(found) = body {
                block(inner, found);
            }
        },
    );
}

fn for_header(builder: &mut Builder<'_>, header: Option<Node<'_>>) {
    match header {
        Some(clause) if clause.kind() == RANGE_CLAUSE => {
            builder.expressions(field(clause, "left"));
            builder.optional(field(clause, "right"));
        }
        Some(clause) if clause.kind() == FOR_CLAUSE => {
            initializer(builder, clause, "initializer");
            builder.optional(field(clause, "condition"));
            initializer(builder, clause, "update");
        }
        condition => builder.optional(condition),
    }
}

fn switch_parts(builder: &mut Builder<'_>, node: Node<'_>) {
    initializer(builder, node, "initializer");
    builder.optional(field(node, "value"));
    cases_block(builder, node, CASE_CLAUSE, CaseLabels::Values);
}

fn type_switch_parts(builder: &mut Builder<'_>, node: Node<'_>) {
    initializer(builder, node, "initializer");
    let value = field(node, "value");
    let assertion = (
        value.map_or_else(|| node.start_byte(), |found| found.start_byte()),
        token(node, ")").map_or_else(|| node.end_byte(), |paren| paren.end_byte()),
    );
    match field(node, "alias") {
        Some(alias) => builder.wrap(ASSIGN_STMT, (alias.start_byte(), assertion.1), |inner| {
            type_assertion(inner, assertion, value);
            inner.expressions(Some(alias));
        }),
        None => builder.wrap(EXPR_STMT, assertion, |inner| {
            type_assertion(inner, assertion, value);
        }),
    }
    cases_block(builder, node, CASE_CLAUSE, CaseLabels::Types);
}

fn type_assertion(builder: &mut Builder<'_>, span: (usize, usize), value: Option<Node<'_>>) {
    builder.wrap(TYPE_ASSERT_EXPR, span, |inner| inner.optional(value));
}

fn cases_block(builder: &mut Builder<'_>, node: Node<'_>, kind: i32, labels: CaseLabels) {
    let start = token_start(node, "{");
    let cases: Vec<Node<'_>> = significant_children(node)
        .into_iter()
        .filter(|child| child.start_byte() >= start)
        .collect();
    builder.wrap(BLOCK_STMT, (start, node.end_byte()), |inner| {
        for case in cases {
            inner.wrap(kind, (case.start_byte(), body_end(case)), |clause| {
                case_parts(clause, case, labels);
            });
        }
    });
}

fn case_parts(builder: &mut Builder<'_>, case: Node<'_>, labels: CaseLabels) {
    match labels {
        CaseLabels::Values => builder.expressions(field(case, "value")),
        CaseLabels::Types => type_labels(builder, case),
        CaseLabels::Communication => initializer(builder, case, "communication"),
    }
    for child in significant_children(case) {
        if child.kind() == STATEMENT_LIST {
            statements(builder, child);
        }
    }
}

fn type_labels(builder: &mut Builder<'_>, case: Node<'_>) {
    let mut cursor = case.walk();
    let types: Vec<Node<'_>> = case
        .children_by_field_name("type", &mut cursor)
        .filter(Node::is_named)
        .collect();
    for listed in types {
        builder.expression(listed);
    }
}
