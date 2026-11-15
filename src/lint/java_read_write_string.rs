use tree_sitter::Node;

use super::java::{IDENTIFIER, NAME_FIELD, TYPE_FIELD, VARIABLE_DECLARATOR, text, unparenthesized};
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const READ_STRING_RELEASE: u32 = 11;
const WRITE_STRING_RELEASE: u32 = 12;
const INVOCATION: &str = "method_invocation";
const OBJECT_CREATION: &str = "object_creation_expression";
const FIELD_ACCESS: &str = "field_access";
const CLASS_BODY: &str = "class_body";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const VALUE_FIELD: &str = "value";
const COMMENT_SUFFIX: &str = "comment";
const FILES: [&str; 2] = ["Files", "java.nio.file.Files"];
const STRINGS: [&str; 2] = ["String", "java.lang.String"];
const CHARSET_HOLDERS: [&str; 2] = ["StandardCharsets", "java.nio.charset.StandardCharsets"];
const CHARSET_CLASSES: [&str; 2] = ["Charset", "java.nio.charset.Charset"];
const CHARSET_FACTORIES: [&str; 2] = ["forName", "defaultCharset"];
const READ_ALL_BYTES: &str = "readAllBytes";
const WRITE: &str = "write";
const GET_BYTES: &str = "getBytes";

// `ReadWriteStringCanBeUsed` de IntelliJ: `new String(Files.readAllBytes(ruta), juego)` donde
// alcanza con `Files.readString(ruta, juego)` (Java 11), y `Files.write(ruta, texto.getBytes(juego))`
// donde alcanza con `Files.writeString` (Java 12; antes IntelliJ sólo lo sugiere fuera de UTF-8). El
// juego de caracteres tiene que ser un `Charset`: una constante de `StandardCharsets`,
// `Charset.forName` o una variable declarada `Charset`.
pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    let since = |needed: u32| release.is_none_or(|release| release >= needed);
    descendants(root)
        .into_iter()
        .filter_map(|node| match node.kind() {
            OBJECT_CREATION if since(READ_STRING_RELEASE) => read_string(node, source),
            INVOCATION if since(WRITE_STRING_RELEASE) => write_string(node, source),
            _ => None,
        })
        .collect()
}

fn arguments(call: Node<'_>) -> Vec<Node<'_>> {
    call.child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !argument.kind().ends_with(COMMENT_SUFFIX))
        .collect()
}

fn read_string(creation: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let created = creation.child_by_field_name(TYPE_FIELD)?;
    let anonymous = named_children(creation)
        .iter()
        .any(|child| child.kind() == CLASS_BODY);
    let [bytes, charset] = arguments(creation)[..] else {
        return None;
    };
    let reads = is_files_call(unparenthesized(bytes), READ_ALL_BYTES, source)
        && arguments(unparenthesized(bytes)).len() == 1;
    (STRINGS.contains(&text(created, source)) && !anonymous && reads && is_charset(charset, source)).then(
        || {
            finding(
                LintKind::Java(JavaRule::ReadWriteString),
                created,
                text(creation, source),
                None,
            )
        },
    )
}

fn write_string(call: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    if !is_files_call(call, WRITE, source) {
        return None;
    }
    let listed = arguments(call);
    let bytes = resolved(*listed.get(1)?, source);
    let encodes = bytes.kind() == INVOCATION
        && bytes
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == GET_BYTES)
        && bytes.child_by_field_name(OBJECT_FIELD).is_some()
        && matches!(arguments(bytes)[..], [charset] if is_charset(charset, source));
    let name = call.child_by_field_name(NAME_FIELD)?;
    encodes.then(|| finding(LintKind::Java(JavaRule::ReadWriteString), name, WRITE, None))
}

fn is_files_call(call: Node<'_>, method: &str, source: &[u8]) -> bool {
    call.kind() == INVOCATION
        && call
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == method)
        && call
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|receiver| FILES.contains(&text(receiver, source)))
}

// `ExpressionUtils.resolveExpression`: una variable local se reemplaza por lo que la inicializa.
fn resolved<'tree>(expression: Node<'tree>, source: &[u8]) -> Node<'tree> {
    let expression = unparenthesized(expression);
    if expression.kind() != IDENTIFIER {
        return expression;
    }
    let name = text(expression, source);
    std::iter::successors(expression.parent(), Node::parent)
        .find_map(|scope| {
            named_children(scope)
                .into_iter()
                .filter(|statement| {
                    statement.kind() == LOCAL_VARIABLE && statement.end_byte() <= expression.start_byte()
                })
                .flat_map(named_children)
                .filter(|declarator| declarator.kind() == VARIABLE_DECLARATOR)
                .find(|declarator| {
                    declarator
                        .child_by_field_name(NAME_FIELD)
                        .is_some_and(|declared| text(declared, source) == name)
                })
                .and_then(|declarator| declarator.child_by_field_name(VALUE_FIELD))
        })
        .map_or(expression, unparenthesized)
}

fn is_charset(expression: Node<'_>, source: &[u8]) -> bool {
    let expression = unparenthesized(expression);
    match expression.kind() {
        FIELD_ACCESS => expression
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|holder| CHARSET_HOLDERS.contains(&text(holder, source))),
        INVOCATION => {
            expression
                .child_by_field_name(OBJECT_FIELD)
                .is_some_and(|holder| CHARSET_CLASSES.contains(&text(holder, source)))
                && expression
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| CHARSET_FACTORIES.contains(&text(name, source)))
        }
        IDENTIFIER => declared_type(expression, text(expression, source), source)
            .is_some_and(|written| CHARSET_CLASSES.contains(&text(written, source))),
        _ => false,
    }
}
