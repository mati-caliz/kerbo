use tree_sitter::Node;

use super::java::{IDENTIFIER, METHOD, NAME_FIELD, PROGRAM, STATIC, children, qualified_name, text};
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const LIST_SORT_RELEASE: u32 = 8;
const INVOCATION: &str = "method_invocation";
const IMPORT: &str = "import_declaration";
const ASTERISK: &str = "asterisk";
const FIELD_ACCESS: &str = "field_access";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const SORT: &str = "sort";
const COLLECTIONS: &str = "Collections";
const COLLECTIONS_CLASS: &str = "java.util.Collections";
const JAVA_UTIL: &str = "java.util";
const STATIC_SORT: &str = "java.util.Collections.sort";
const ARRAYS_CLASS: &str = "java.util.Arrays";
const STATIC_ARRAYS_SORT: &str = "java.util.Arrays.sort";
const ARRAY_TYPE: &str = "array_type";
const ARRAY_CREATION: &str = "array_creation_expression";
const TO_ARRAY: &str = "toArray";
const SORT_ARGUMENTS: usize = 2;
const COMMENT_SUFFIX: &str = "comment";
const STAR_SUFFIX: &str = ".*";
const LOOSER_THAN_POSTFIX: [&str; 9] = [
    "cast_expression",
    "ternary_expression",
    "binary_expression",
    "unary_expression",
    "update_expression",
    "lambda_expression",
    "assignment_expression",
    "instanceof_expression",
    "switch_expression",
];

#[derive(Debug)]
struct Import {
    name: String,
    is_static: bool,
    is_star: bool,
}

fn imports_member(imports: &[Import], is_static: bool, single: &str, owner: &str) -> bool {
    imports.iter().any(|import| {
        import.is_static == is_static
            && if import.is_star {
                import.name == owner
            } else {
                import.name == single
            }
    })
}

pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if root.kind() != PROGRAM || release.is_some_and(|release| release < LIST_SORT_RELEASE) {
        return Vec::new();
    }
    let imports = imports(root, source);
    let imports_collections = imports_member(&imports, false, COLLECTIONS_CLASS, JAVA_UTIL);
    let imports_sort = imports_member(&imports, true, STATIC_SORT, COLLECTIONS_CLASS);
    let imports_arrays_sort = imports_member(&imports, true, STATIC_ARRAYS_SORT, ARRAYS_CLASS);
    let nodes = descendants(root);
    let declares_sort = nodes
        .iter()
        .any(|node| node.kind() == METHOD && name_is(*node, SORT, source));
    let declares_collections = nodes
        .iter()
        .any(|node| super::java::is_type_declaration(*node) && name_is(*node, COLLECTIONS, source));
    nodes
        .into_iter()
        .filter(|node| node.kind() == INVOCATION && name_is(*node, SORT, source))
        .filter(|invocation| {
            invocation
                .child_by_field_name(OBJECT_FIELD)
                .map_or(imports_sort && !declares_sort, |object| match object.kind() {
                    IDENTIFIER => {
                        text(object, source) == COLLECTIONS && imports_collections && !declares_collections
                    }
                    FIELD_ACCESS => qualified_name(object, source) == COLLECTIONS_CLASS,
                    _ => false,
                })
        })
        .filter_map(|invocation| {
            let [list, comparator] = arguments(invocation)?;
            let unqualified = invocation.child_by_field_name(OBJECT_FIELD).is_none();
            if unqualified && imports_arrays_sort && is_array(list, source) {
                return None;
            }
            let name = invocation.child_by_field_name(NAME_FIELD)?;
            let replacement = format!("{}.sort({})", receiver(list, source), text(comparator, source));
            Some(finding(
                LintKind::Java(JavaRule::Java8ListSort),
                name,
                text(list, source),
                Some(Fix::replace(invocation, &replacement, source)),
            ))
        })
        .collect()
}

fn imports(root: Node<'_>, source: &[u8]) -> Vec<Import> {
    named_children(root)
        .into_iter()
        .filter(|declaration| declaration.kind() == IMPORT)
        .map(|declaration| {
            let tokens = children(declaration);
            Import {
                name: qualified_name(declaration, source)
                    .trim_end_matches(STAR_SUFFIX)
                    .to_owned(),
                is_static: tokens.iter().any(|token| token.kind() == STATIC),
                is_star: tokens.iter().any(|token| token.kind() == ASTERISK),
            }
        })
        .collect()
}

fn name_is(node: Node<'_>, wanted: &str, source: &[u8]) -> bool {
    node.child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == wanted)
}

fn arguments(invocation: Node<'_>) -> Option<[Node<'_>; SORT_ARGUMENTS]> {
    let listed: Vec<Node<'_>> = named_children(invocation.child_by_field_name(ARGUMENTS_FIELD)?)
        .into_iter()
        .filter(|argument| !argument.kind().ends_with(COMMENT_SUFFIX))
        .collect();
    match listed.as_slice() {
        [list, comparator] => Some([*list, *comparator]),
        _ => None,
    }
}

fn is_array(argument: Node<'_>, source: &[u8]) -> bool {
    match argument.kind() {
        ARRAY_CREATION => true,
        IDENTIFIER => declared_type(argument, text(argument, source), source)
            .is_some_and(|written| written.kind() == ARRAY_TYPE),
        INVOCATION => name_is(argument, TO_ARRAY, source),
        _ => false,
    }
}

fn receiver(list: Node<'_>, source: &[u8]) -> String {
    let written = text(list, source);
    if LOOSER_THAN_POSTFIX.contains(&list.kind()) {
        format!("({written})")
    } else {
        written.to_owned()
    }
}
