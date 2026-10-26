use tree_sitter::Node;

use super::builtin_calls::{Arguments, keyword_argument};
use super::call_fixes::builtin_fix;
use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::model::Model;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{VALUE_FIELD, text};
use crate::lint::python::fix_edits::commented_unsafe;

const MEMBER_SEPARATOR: char = '.';
const COLLECTIONS: &str = "collections";
const PEP585_BUILTINS: [(&str, &str); 6] = [
    ("Dict", "dict"),
    ("FrozenSet", "frozenset"),
    ("List", "list"),
    ("Set", "set"),
    ("Tuple", "tuple"),
    ("Type", "type"),
];
const PEP585_COLLECTIONS: [(&str, &str); 2] = [("Deque", "deque"), ("DefaultDict", "defaultdict")];
const DATETIME: &str = "datetime";
const UTC: &str = "UTC";
const RE: &str = "re";
const REGEX_FLAGS: [(&str, &str); 8] = [
    ("A", "ASCII"),
    ("I", "IGNORECASE"),
    ("L", "LOCALE"),
    ("M", "MULTILINE"),
    ("S", "DOTALL"),
    ("T", "TEMPLATE"),
    ("U", "UNICODE"),
    ("X", "VERBOSE"),
];
const LOGGING: &str = "logging";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const SYS: &str = "sys";
const EXIT: &str = "exit";
const EXIT_CODE: &str = "code";
const WARNING: &str = "WARNING";

// UP006: `typing.List` pasa a `list`, y `typing.Deque` a `deque` de `collections`.
pub fn pep585_fix(model: &Model<'_>, node: Node<'_>, qualified: &str, source: &[u8]) -> Option<Fix> {
    let member = qualified.rsplit(MEMBER_SEPARATOR).next()?;
    if let Some((_, builtin)) = PEP585_BUILTINS.iter().find(|(alias, _)| *alias == member) {
        return builtin_fix(model, node, builtin, source);
    }
    let (_, replacement) = PEP585_COLLECTIONS.iter().find(|(alias, _)| *alias == member)?;
    imported_fix(model, node, import_from(COLLECTIONS, replacement), source)
}

// El builtin de PEP 585 para `typing.List` y compañía; `None` para los de `collections`.
pub fn pep585_builtin(qualified: &str) -> Option<&'static str> {
    let member = qualified.rsplit(MEMBER_SEPARATOR).next()?;
    PEP585_BUILTINS
        .iter()
        .find(|(alias, _)| *alias == member)
        .map(|(_, builtin)| *builtin)
}

// UP017: `datetime.timezone.utc` pasa a `UTC` de `datetime`; inseguro con comentarios adentro.
pub fn utc_fix(model: &Model<'_>, node: Node<'_>, source: &[u8]) -> Option<Fix> {
    let fix = imported_fix(model, node, import_from(DATETIME, UTC), source)?;
    Some(commented_unsafe(fix, node, source))
}

// FURB167: `re.I` pasa a `re.IGNORECASE`.
pub fn regex_flag_fix(model: &Model<'_>, node: Node<'_>, qualified: &str, source: &[u8]) -> Option<Fix> {
    let flag = qualified.rsplit(MEMBER_SEPARATOR).next()?;
    let (_, full) = REGEX_FLAGS.iter().find(|(alias, _)| *alias == flag)?;
    imported_fix(model, node, import_module(RE, full), source)
}

// LOG009: `logging.WARN` pasa a `logging.WARNING`.
pub fn warn_constant_fix(model: &Model<'_>, node: Node<'_>, source: &[u8]) -> Option<Fix> {
    imported_fix(model, node, import_module(LOGGING, WARNING), source)
}

// PLR1722: `exit(1)` pasa a `sys.exit(1)`, y `code=1` a `1`. Siempre inseguro, como en ruff; sin
// arreglo con más de un argumento o con `**`.
pub fn sys_exit_fix(
    model: &Model<'_>,
    function: Node<'_>,
    arguments: &Arguments<'_>,
    source: &[u8],
) -> Option<Fix> {
    let count = arguments
        .positional
        .len()
        .saturating_add(arguments.keywords.len());
    let spread = arguments
        .keywords
        .iter()
        .any(|keyword| keyword.kind() == DICTIONARY_SPLAT);
    if count > 1 || spread {
        return None;
    }
    let (name, import) = get_or_import_member(model, source, function, import_module(SYS, EXIT))?;
    let mut edits = vec![TextEdit {
        start: TextPoint::from_point(function.start_position()),
        end: TextPoint::from_point(function.end_position()),
        replacement: name,
    }];
    if let Some(keyword) = keyword_argument(arguments, EXIT_CODE, source)
        && let Some(value) = keyword.child_by_field_name(VALUE_FIELD)
    {
        edits.push(TextEdit {
            start: TextPoint::from_point(keyword.start_position()),
            end: TextPoint::from_point(keyword.end_position()),
            replacement: text(value, source).to_owned(),
        });
    }
    edits.push(import);
    Some(Fix::from_edits(source, edits).marked_unsafe())
}

const fn import_from<'request>(module: &'request str, member: &'request str) -> ImportRequest<'request> {
    ImportRequest {
        module,
        member,
        style: ImportStyle::ImportFrom,
    }
}

const fn import_module<'request>(module: &'request str, member: &'request str) -> ImportRequest<'request> {
    ImportRequest {
        module,
        member,
        style: ImportStyle::Import,
    }
}

pub fn imported_fix(
    model: &Model<'_>,
    node: Node<'_>,
    request: ImportRequest<'_>,
    source: &[u8],
) -> Option<Fix> {
    let (name, import) = get_or_import_member(model, source, node, request)?;
    let reference = TextEdit {
        start: TextPoint::from_point(node.start_position()),
        end: TextPoint::from_point(node.end_position()),
        replacement: name,
    };
    Some(Fix::from_edits(source, vec![reference, import]))
}
