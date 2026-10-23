use tree_sitter::Node;

pub const MODULE: &str = "module";
pub const IDENTIFIER: &str = "identifier";
pub const ATTRIBUTE: &str = "attribute";
pub const SUBSCRIPT: &str = "subscript";
pub const CALL: &str = "call";
pub const STRING: &str = "string";
pub const CONCATENATED_STRING: &str = "concatenated_string";
pub const STRING_START: &str = "string_start";
pub const INTERPOLATION: &str = "interpolation";
pub const COMMENT: &str = "comment";
pub const BLOCK: &str = "block";
pub const PARENTHESIZED: &str = "parenthesized_expression";
pub const TUPLE: &str = "tuple";
pub const LIST: &str = "list";
pub const SET: &str = "set";
pub const LAMBDA: &str = "lambda";
pub const ASSIGNMENT: &str = "assignment";
pub const AUGMENTED_ASSIGNMENT: &str = "augmented_assignment";
pub const EXPRESSION_STATEMENT: &str = "expression_statement";
pub const FUNCTION: &str = "function_definition";
pub const CLASS: &str = "class_definition";
pub const DECORATED: &str = "decorated_definition";
pub const IMPORT: &str = "import_statement";
pub const IMPORT_FROM: &str = "import_from_statement";
pub const FUTURE_IMPORT: &str = "future_import_statement";
pub const ALIASED_IMPORT: &str = "aliased_import";
pub const DOTTED_NAME: &str = "dotted_name";
pub const WILDCARD_IMPORT: &str = "wildcard_import";
pub const COMPARISON: &str = "comparison_operator";
pub const NOT_OPERATOR: &str = "not_operator";
pub const KEYWORD_ARGUMENT: &str = "keyword_argument";
pub const NAME_FIELD: &str = "name";
pub const ALIAS_FIELD: &str = "alias";
pub const BODY_FIELD: &str = "body";
pub const VALUE_FIELD: &str = "value";
pub const LEFT_FIELD: &str = "left";
pub const RIGHT_FIELD: &str = "right";
pub const TYPE_FIELD: &str = "type";
pub const OPERATORS_FIELD: &str = "operators";
pub const ARGUMENT_FIELD: &str = "argument";
pub const FUNCTION_FIELD: &str = "function";
pub const ARGUMENTS_FIELD: &str = "arguments";
pub const OBJECT_FIELD: &str = "object";
pub const ATTRIBUTE_FIELD: &str = "attribute";
pub const SUBSCRIPT_FIELD: &str = "subscript";
pub const MODULE_NAME_FIELD: &str = "module_name";
pub const NONE: &str = "none";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";

const KEYWORDS: [&str; 35] = [
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def",
    "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield",
];

pub fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

pub fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

pub fn named(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| !child.is_extra())
        .collect()
}

pub fn fields<'tree>(node: Node<'tree>, field: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

pub fn unparenthesized(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        match named(current).as_slice() {
            [inner] => current = *inner,
            _ => return current,
        }
    }
    current
}

pub fn is_string(node: Node<'_>) -> bool {
    matches!(node.kind(), STRING | CONCATENATED_STRING)
}

pub fn string_prefix<'source>(string: Node<'_>, source: &'source [u8]) -> &'source str {
    string
        .child(0)
        .filter(|start| start.kind() == STRING_START)
        .map(|start| text(start, source).trim_end_matches(['"', '\'']))
        .unwrap_or_default()
}

pub fn is_f_string(string: Node<'_>, source: &[u8]) -> bool {
    string_prefix(string, source).contains(['f', 'F'])
}

pub fn is_bytes(string: Node<'_>, source: &[u8]) -> bool {
    string_prefix(string, source).contains(['b', 'B'])
}

pub fn statement_ancestors(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
}

pub fn is_keyword(name: &str) -> bool {
    KEYWORDS.contains(&name)
}

pub fn is_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    let starts = characters
        .next()
        .is_some_and(|first| first == '_' || first.is_alphabetic());
    starts
        && characters.all(|character| character == '_' || character.is_alphanumeric())
        && !KEYWORDS.contains(&name)
}
