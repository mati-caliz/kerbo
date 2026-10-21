use tree_sitter::{Node, Point};

const LINE_FEED: u8 = b'\n';
const SWALLOWED: &[char] = &[' ', '\t', '\n', '\r', '\\'];
pub const PROGRAM: &str = "program";
pub const COMMENT: &str = "comment";
pub const COMMAND: &str = "command";
pub const COMMAND_NAME: &str = "command_name";
pub const DECLARATION: &str = "declaration_command";
pub const UNSET: &str = "unset_command";
pub const ASSIGNMENT: &str = "variable_assignment";
pub const ASSIGNMENTS: &str = "variable_assignments";
pub const REDIRECTED: &str = "redirected_statement";
pub const FILE_REDIRECT: &str = "file_redirect";
pub const HERESTRING: &str = "herestring_redirect";
pub const HEREDOC_REDIRECT: &str = "heredoc_redirect";
pub const HEREDOC_BODY: &str = "heredoc_body";
pub const PIPELINE: &str = "pipeline";
pub const LIST: &str = "list";
pub const SUBSHELL: &str = "subshell";
pub const COMPOUND: &str = "compound_statement";
pub const NEGATED: &str = "negated_command";
pub const TEST: &str = "test_command";
pub const IF: &str = "if_statement";
pub const ELIF_CLAUSE: &str = "elif_clause";
pub const ELSE_CLAUSE: &str = "else_clause";
pub const WHILE: &str = "while_statement";
pub const FOR: &str = "for_statement";
pub const ARITHMETIC_FOR: &str = "c_style_for_statement";
pub const CASE: &str = "case_statement";
pub const CASE_ITEM: &str = "case_item";
pub const DO_GROUP: &str = "do_group";
pub const FUNCTION: &str = "function_definition";
pub const WORD: &str = "word";
pub const NUMBER: &str = "number";
pub const STRING: &str = "string";
pub const STRING_CONTENT: &str = "string_content";
pub const RAW_STRING: &str = "raw_string";
pub const ANSI_STRING: &str = "ansi_c_string";
pub const TRANSLATED_STRING: &str = "translated_string";
pub const CONCATENATION: &str = "concatenation";
pub const SIMPLE_EXPANSION: &str = "simple_expansion";
pub const EXPANSION: &str = "expansion";
pub const COMMAND_SUBSTITUTION: &str = "command_substitution";
pub const PROCESS_SUBSTITUTION: &str = "process_substitution";
pub const ARITHMETIC_EXPANSION: &str = "arithmetic_expansion";
pub const ARRAY: &str = "array";
pub const SUBSCRIPT: &str = "subscript";
pub const VARIABLE_NAME: &str = "variable_name";
pub const SPECIAL_VARIABLE: &str = "special_variable_name";
pub const BINARY: &str = "binary_expression";
pub const UNARY: &str = "unary_expression";
pub const POSTFIX: &str = "postfix_expression";
pub const PARENTHESIZED: &str = "parenthesized_expression";
pub const TERNARY: &str = "ternary_expression";
pub const TEST_OPERATOR: &str = "test_operator";
pub const ARGUMENT_FIELD: &str = "argument";
pub const NAME_FIELD: &str = "name";
pub const VALUE_FIELD: &str = "value";
pub const BODY_FIELD: &str = "body";
pub const REDIRECT_FIELD: &str = "redirect";
pub const DESTINATION_FIELD: &str = "destination";
pub const DESCRIPTOR_FIELD: &str = "descriptor";
pub const OPERATOR_FIELD: &str = "operator";
pub const CONDITION_FIELD: &str = "condition";
pub const LEFT_FIELD: &str = "left";
pub const RIGHT_FIELD: &str = "right";
pub const VARIABLE_FIELD: &str = "variable";
pub const INDEX_FIELD: &str = "index";
pub const BACKTICK: &str = "`";
pub const DOUBLE_PARENTHESIS: &str = "((";
pub const DOUBLE_BRACKET: &str = "[[";
pub const BACKGROUND: &str = "&";
pub const AND: &str = "&&";
pub const OR: &str = "||";
pub const EXPANSIONS: &[&str] = &[SIMPLE_EXPANSION, EXPANSION];
pub const STATEMENT_KINDS: &[&str] = &[
    COMMAND,
    DECLARATION,
    UNSET,
    TEST,
    COMPOUND,
    SUBSHELL,
    IF,
    WHILE,
    FOR,
    ARITHMETIC_FOR,
    CASE,
    FUNCTION,
    REDIRECTED,
];

pub fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

pub fn token_text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    text(node, source).trim_start_matches(SWALLOWED)
}

pub fn token_start(byte: usize, point: Point, source: &[u8]) -> (usize, Point) {
    let mut byte = byte;
    let mut point = point;
    while let Some(&current) = source
        .get(byte)
        .filter(|current| SWALLOWED.contains(&char::from(**current)))
    {
        byte = byte.saturating_add(1);
        point = if current == LINE_FEED {
            Point {
                row: point.row.saturating_add(1),
                column: 0,
            }
        } else {
            Point {
                row: point.row,
                column: point.column.saturating_add(1),
            }
        };
    }
    (byte, point)
}

pub fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

pub fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

pub fn field_children<'tree>(node: Node<'tree>, field: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

pub fn first_token<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.child(0).map_or("", |first| token_text(first, source))
}

pub fn is_arithmetic_command(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == COMPOUND && first_token(node, source) == DOUBLE_PARENTHESIS
}

pub fn is_backticked(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == COMMAND_SUBSTITUTION && first_token(node, source) == BACKTICK
}

pub fn is_double_bracket(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == TEST && first_token(node, source) == DOUBLE_BRACKET
}

pub fn is_backgrounded(node: Node<'_>, source: &[u8]) -> bool {
    node.next_sibling()
        .is_some_and(|next| !next.is_named() && text(next, source) == BACKGROUND)
}

pub fn is_statement(node: Node<'_>) -> bool {
    match node.kind() {
        ASSIGNMENT | ASSIGNMENTS => node
            .parent()
            .is_none_or(|parent| ![COMMAND, DECLARATION, ASSIGNMENTS].contains(&parent.kind())),
        kind => STATEMENT_KINDS.contains(&kind),
    }
}

pub fn ancestors(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
}

pub fn descendants(node: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        found.push(current);
        pending.extend(children(current).into_iter().rev());
    }
    found
}
