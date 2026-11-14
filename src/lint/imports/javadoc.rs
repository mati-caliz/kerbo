use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use crate::syntax::descendants;

const BLOCK_COMMENT: &str = "block_comment";
const LINE_COMMENT: &str = "line_comment";
const JAVADOC_START: &str = "/**";
const MARKDOWN_START: &str = "///";
const COMMENT_END: &str = "*/";
const LINE_DECORATION: char = '*';
const NAME_SEPARATOR: char = '.';
const PARAMETER_SEPARATOR: char = ',';
const VARARGS_SUFFIX: &str = "...";
const ARRAY_SUFFIX: &str = "[]";
const TYPE_PART: &str = r"((?:[A-Za-z][0-9A-Za-z_]*\.)*(?:[A-Za-z][0-9A-Za-z_]*))?(?:#[0-9A-Za-z_]*(?:\(([.0-9A-Za-z_\t\n\x0B\x0C\r ,\[\]]*)\))?)?";
const SPACES: &str = r"[\t\n\x0B\x0C\r ]+";
const SIMPLE_NAME: &str = "([A-Za-z][0-9A-Za-z_]*)";
const PATTERNS: [[&str; 5]; 7] = [
    ["@see", SPACES, "", TYPE_PART, ""],
    [r"\{@link(?:plain)?", SPACES, "", TYPE_PART, r"[\t\n\x0B\x0C\r }]"],
    [r"\{@value", SPACES, "", SIMPLE_NAME, r"[\t\n\x0B\x0C\r #}]"],
    ["@throws", SPACES, "", SIMPLE_NAME, ""],
    ["@exception", SPACES, "", SIMPLE_NAME, ""],
    [
        r"//[\t\n\x0B\x0C\r ]*@link",
        SPACES,
        r#"(?:.*?)?target=["']?"#,
        TYPE_PART,
        r#"["']?"#,
    ],
    [r"\[", "", "", TYPE_PART, r"\]"],
];

static REFERENCES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    PATTERNS
        .iter()
        .filter_map(|parts| Regex::new(&parts.concat()).ok())
        .collect()
});

pub fn referenced_names(root: Node<'_>, source: &[u8]) -> HashSet<String> {
    descendants(root)
        .into_iter()
        .filter(|node| [BLOCK_COMMENT, LINE_COMMENT].contains(&node.kind()))
        .filter_map(|comment| comment.utf8_text(source).ok())
        .filter(|text| is_documentation(text))
        .flat_map(|text| names_in(&filtered(text)))
        .collect()
}

fn is_documentation(text: &str) -> bool {
    (text.starts_with(JAVADOC_START) && text != "/**/") || text.starts_with(MARKDOWN_START)
}

fn filtered(text: &str) -> String {
    text.lines()
        .map(|line| {
            let line = line.trim();
            let line = line
                .strip_prefix(JAVADOC_START)
                .or_else(|| line.strip_prefix(MARKDOWN_START))
                .unwrap_or(line);
            let line = line.strip_suffix(COMMENT_END).unwrap_or(line);
            line.trim_start_matches(LINE_DECORATION).trim()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn names_in(text: &str) -> Vec<String> {
    REFERENCES
        .iter()
        .flat_map(|pattern| pattern.captures_iter(text))
        .flat_map(|captures| {
            let named = captures.get(1).map(|name| name.as_str().to_owned());
            let parameters = captures
                .get(2)
                .map(|list| {
                    list.as_str()
                        .split(PARAMETER_SEPARATOR)
                        .map(|parameter| parameter_type(parameter.trim()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            named.into_iter().chain(parameters)
        })
        .map(|name| first_segment(&name).to_owned())
        .collect()
}

fn parameter_type(parameter: &str) -> String {
    let mut name = parameter.strip_suffix(VARARGS_SUFFIX).unwrap_or(parameter);
    while let Some(shorter) = name.strip_suffix(ARRAY_SUFFIX) {
        name = shorter;
    }
    name.to_owned()
}

fn first_segment(name: &str) -> &str {
    name.split(NAME_SEPARATOR).next().unwrap_or(name)
}
