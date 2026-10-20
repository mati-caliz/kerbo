use std::sync::LazyLock;

use regex::{Regex, RegexSet};
use tree_sitter::Node;

use super::python_tokens::has_consecutive_names;
use super::{CommentedCode, SnippetLanguage, at, parse_snippet};
use crate::syntax::{descendants, named_children};

const COMMENT: &str = "comment";
const SCRIPT_TAG: &str = "# /// script";
const SCRIPT_END: &str = "///";
const PYTHON_WHITESPACE: &[char] = &[' ', '\t', '\u{c}'];
const CODE_INDICATORS: &[&str] = &[
    "(", ")", "[", "]", "{", "}", ":", "=", "%", "return", "break", "continue", "import",
];
const TASK_TAGS: &[&str] = &["TODO", "FIXME", "XXX"];
const TASK_TAG_SEPARATORS: &[char] = &[' ', ':', '('];
const PYTHON_TWO_ONLY: &[&str] = &["print_statement", "exec_statement"];
const BLOCK: &str = "block";
const ASSIGNMENT: &str = "assignment";
const SINGLE_TARGETS: &[&str] = &["identifier", "attribute", "subscript"];
const AS_PATTERN: &str = "as_pattern";
const AS_PATTERN_OWNERS: &[&str] = &["with_item", "with_clause", "except_clause", "case_clause"];
const STRING_START: &str = "string_start";
const PAIR: &str = "pair";
const CONSTRAINED_TYPE: &str = "constrained_type";
const TYPE_PARAMETER: &str = "type_parameter";
const NAMED_EXPRESSION: &str = "named_expression";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const DICTIONARY: &str = "dictionary";

// Las expresiones de ruff (crates/ruff_linter/src/rules/eradicate/detection.rs, 0.16.3), tal cual.
static ALLOWLIST: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (?:
            pyright
        |   pyrefly
        |   ruff\s*:\s*(disable|enable|ignore|file-ignore)
        |   mypy:
        |   type:\s*ignore
        |   ty:\s*ignore
        |   SPDX-License-Identifier:
        |   fmt:\s*(on|off|skip)
        |   region|endregion
        |   (?i:
                noqa
            )
        |   (?i:
                pylint
            |   nosec
            |   isort:\s*(on|off|skip|skip_file|split|dont-add-imports(:\s*\[.*?])?)
            |   (?:en)?coding[:=][\x20\t]*([-_.A-Z0-9]+)
            )
        |   language=[-_.a-zA-Z0-9]+
        )
        ",
    )
    .ok()
});
static HASH_NUMBER: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"#\d").ok());
static POSITIVE_CASES: LazyLock<Option<RegexSet>> = LazyLock::new(|| {
    RegexSet::new([
        r"^(?:elif\s+.*\s*:.*|else\s*:.*|try\s*:.*|finally\s*:.*|except.*:.*|case\s+.*\s*:.*)$",
        r#"^['"]\w+['"]\s*:.+[,{]\s*(#.*)?$"#,
        r"^(?:[(\[]\s*)?(?:\w+\s*,\s*)*\w+\s*([)\]]\s*)?=.*[(\[{]\s*(#.*)?$",
        r"^[()\[\]{}\s]+\s*(#.*)?$",
    ])
    .ok()
});

pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<CommentedCode> {
    let text = String::from_utf8_lossy(source);
    let comments: Vec<Node<'_>> = descendants(root)
        .into_iter()
        .filter(|node| node.kind() == COMMENT)
        .collect();
    let mut found = Vec::new();
    let mut skip_until = 0;
    for comment in comments {
        if comment.start_byte() < skip_until {
            continue;
        }
        let line = line_of(&text, comment.start_byte());
        if line == SCRIPT_TAG
            && let Some(end) = script_end(&text, comment.start_byte())
        {
            skip_until = end;
            continue;
        }
        if is_own_line(line) && contains_code(line) {
            found.push(at(comment, source));
        }
    }
    found
}

fn line_of(text: &str, offset: usize) -> &str {
    let start = text
        .get(..offset)
        .and_then(|before| before.rfind(['\n', '\r']))
        .map_or(0, |newline| newline.saturating_add(1));
    let rest = text.get(start..).unwrap_or_default();
    rest.find(['\n', '\r'])
        .map_or(rest, |end| rest.get(..end).unwrap_or_default())
}

fn script_end(text: &str, tag_start: usize) -> Option<usize> {
    let line_end = text
        .get(tag_start..)?
        .find('\n')
        .map(|offset| tag_start.saturating_add(offset).saturating_add(1))?;
    let mut offset = line_end;
    let mut end = None;
    for line in text.get(line_end..)?.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        let Some(rest) = content.strip_prefix('#') else {
            break;
        };
        let script_content = if rest.is_empty() {
            Some("")
        } else {
            rest.strip_prefix(' ')
        };
        let Some(script_content) = script_content else {
            break;
        };
        offset = offset.saturating_add(line.len());
        if script_content == SCRIPT_END {
            end = Some(offset);
        }
    }
    end
}

fn is_own_line(line: &str) -> bool {
    line.chars().find(|character| !character.is_whitespace()) == Some('#')
}

fn contains_code(line: &str) -> bool {
    let line = line
        .trim_start_matches(|character: char| character == '#' || PYTHON_WHITESPACE.contains(&character))
        .trim_end();
    if !CODE_INDICATORS.iter().any(|indicator| line.contains(indicator)) || has_consecutive_names(line) {
        return false;
    }
    if line
        .split(TASK_TAG_SEPARATORS)
        .next()
        .is_some_and(|first| TASK_TAGS.contains(&first))
    {
        return false;
    }
    if matches_regex(&ALLOWLIST, line) || matches_regex(&HASH_NUMBER, line) {
        return false;
    }
    if line.ends_with('\\') {
        return true;
    }
    if POSITIVE_CASES.as_ref().is_some_and(|cases| cases.is_match(line)) {
        return true;
    }
    parses_as_python(line)
}

fn matches_regex(regex: &LazyLock<Option<Regex>>, line: &str) -> bool {
    regex.as_ref().is_some_and(|compiled| compiled.is_match(line))
}

// ruff termina compilando la línea con su parser de Python 3, y tree-sitter no acepta lo mismo:
// suma las sentencias print y exec y los strings entre comillas invertidas de Python 2, un bloque
// vacío después de `if x:` o `def f():` (la línea sola, sin cuerpo), `a as b` fuera de un `with`,
// un `except` o un `case`, `x := 1` sin paréntesis, `a: b: c` fuera de un parámetro de tipo y
// una anotación sobre algo que no es un único destino (`a, b: int`).
// Al revés, sólo acepta como anotación un tipo, no cualquier expresión: `x: a[1] is b` se
// prueba como el par de un diccionario.
fn parses_as_python(line: &str) -> bool {
    parses_cleanly(line).is_some_and(|accepted| accepted) || parses_as_annotation(line)
}

fn parse(text: &str) -> Option<tree_sitter::Tree> {
    parse_snippet(SnippetLanguage::Python, text)
}

fn parses_cleanly(text: &str) -> Option<bool> {
    let tree = parse(text)?;
    let root = tree.root_node();
    Some(
        !root.has_error()
            && !descendants(root)
                .into_iter()
                .any(|node| is_rejected_by_ruff(node, text)),
    )
}

fn parses_as_annotation(line: &str) -> bool {
    let wrapped = format!("{{{line}}}");
    let Some(tree) = parse(&wrapped) else {
        return false;
    };
    let root = tree.root_node();
    if root.has_error()
        || descendants(root)
            .into_iter()
            .any(|node| is_rejected_by_ruff(node, &wrapped))
    {
        return false;
    }
    let statement = named_children(root);
    let [only] = statement.as_slice() else {
        return false;
    };
    let dictionary = named_children(*only);
    let [single] = dictionary.as_slice() else {
        return false;
    };
    let pairs = named_children(*single);
    matches!(pairs.as_slice(), [pair] if pair.kind() == PAIR
        && pair.child_by_field_name("key").is_some_and(|key| SINGLE_TARGETS.contains(&key.kind())))
        && single.kind() == DICTIONARY
}

fn is_rejected_by_ruff(node: Node<'_>, text: &str) -> bool {
    match node.kind() {
        kind if PYTHON_TWO_ONLY.contains(&kind) => true,
        BLOCK => named_children(node).iter().all(|child| child.kind() == COMMENT),
        AS_PATTERN => !has_ancestor(node, AS_PATTERN_OWNERS),
        CONSTRAINED_TYPE => !has_ancestor(node, &[TYPE_PARAMETER]),
        NAMED_EXPRESSION => node
            .parent()
            .is_some_and(|parent| parent.kind() == EXPRESSION_STATEMENT),
        STRING_START => text
            .get(node.start_byte()..node.end_byte())
            .is_some_and(|start| start.contains('`')),
        ASSIGNMENT => {
            node.child_by_field_name("type").is_some()
                && node
                    .child_by_field_name("left")
                    .is_some_and(|target| !SINGLE_TARGETS.contains(&target.kind()))
        }
        _ => false,
    }
}

fn has_ancestor(node: Node<'_>, kinds: &[&str]) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if kinds.contains(&parent.kind()) {
            return true;
        }
        current = parent.parent();
    }
    false
}
