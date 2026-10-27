use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::{Node, Parser, Point};

use crate::language::Language;
use crate::syntax::one_based;

const COMMENT: &str = "comment";
const MARKER: &str = "eslint";
const LINE_PREFIX: &str = "//";
const BLOCK_PREFIX: &str = "/*";
const BLOCK_SUFFIX: &str = "*/";
const CONFIGURE: &str = "eslint";
const DISABLE: &str = "eslint-disable";
const ENABLE: &str = "eslint-enable";
const DISABLE_LINE: &str = "eslint-disable-line";
const DISABLE_NEXT_LINE: &str = "eslint-disable-next-line";
const RULE_SEPARATOR: char = ',';
const FIRST_LINE: usize = 1;
const TYPESCRIPT_PLUGIN: &str = "@typescript-eslint/";
// Las reglas de ESLint que kerbo cubre con sus reglas universales.
const UNIVERSAL_RULES: [(&str, &str); 7] = [
    ("complexity", "cyclomatic-complexity"),
    ("sonarjs/cognitive-complexity", "cognitive-complexity"),
    ("max-lines", "file-length"),
    ("max-lines-per-function", "function-length"),
    ("max-params", "parameter-count"),
    ("max-depth", "nesting-depth"),
    ("no-magic-numbers", "magic-number"),
];

static DIRECTIVE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^(eslint(?:-env|-enable|-disable(?:(?:-next)?-line)?)?|exported|globals?)(?:\s|$)").ok()
});
static TURNED_OFF: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"(?:^|,)\s*["']?(?<rule>[@\w/.-]+)["']?\s*:\s*\[?\s*(?:0|"off"|'off')\s*(?:[,\]]|$)"#).ok()
});
static JUSTIFICATION: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\s-{2,}\s").ok());

// Una posición como las compara ESLint: línea y columna desde 1.
pub type Position = (usize, usize);

// Lo que silencia un comentario de ESLint: las reglas (vacío es todas) desde `start` hasta antes de
// `end`, que es `None` si sigue hasta el final del archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisabledRange {
    pub line: usize,
    pub text: String,
    pub rules: Vec<String>,
    pub reason: String,
    pub start: Position,
    pub end: Option<Position>,
}

struct Comment {
    single_line: bool,
    value: String,
    text: String,
    start: Point,
    end: Point,
}

// Los `eslint-disable-line` y `eslint-disable-next-line` (en cualquier comentario), los
// `/* eslint-disable */` hasta su `/* eslint-enable */` y las reglas apagadas en todo el archivo con
// `/* eslint regla: "off" */`, como `getDirectiveComments` de ESLint.
pub fn disabled_ranges(source: &str, language: Language) -> Vec<DisabledRange> {
    if !source.contains(MARKER) {
        return Vec::new();
    }
    let mut ranges = Vec::new();
    let mut open: Vec<DisabledRange> = Vec::new();
    for comment in comments(source, language) {
        let Some((kind, rules, reason)) = parse(&comment) else {
            continue;
        };
        let line = one_based(comment.start.row);
        let at = (line, one_based(comment.start.column));
        match kind {
            DISABLE_LINE if comment.start.row == comment.end.row => {
                ranges.push(whole_line(line, &comment, rules, reason));
            }
            DISABLE_NEXT_LINE => {
                let next = one_based(comment.end.row).saturating_add(1);
                ranges.push(DisabledRange {
                    line,
                    ..whole_line(next, &comment, rules, reason)
                });
            }
            DISABLE => open.push(DisabledRange {
                line,
                text: comment.text.clone(),
                rules,
                reason,
                start: at,
                end: None,
            }),
            CONFIGURE if !rules.is_empty() => ranges.push(DisabledRange {
                line,
                text: comment.text.clone(),
                rules,
                reason,
                start: (FIRST_LINE, 0),
                end: None,
            }),
            ENABLE => {
                let (closed, still_open): (Vec<DisabledRange>, Vec<DisabledRange>) =
                    open.into_iter().partition(|range| {
                        rules.is_empty() || range.rules.iter().any(|rule| rules.contains(rule))
                    });
                open = still_open;
                ranges.extend(closed.into_iter().map(|range| DisabledRange {
                    end: Some(at),
                    ..range
                }));
            }
            _ => {}
        }
    }
    ranges.extend(open);
    ranges
}

fn whole_line(line: usize, comment: &Comment, rules: Vec<String>, reason: String) -> DisabledRange {
    DisabledRange {
        line,
        text: comment.text.clone(),
        rules,
        reason,
        start: (line, 0),
        end: Some((line.saturating_add(1), 0)),
    }
}

// `extractDirectiveComment` y el patrón de directivas: lo de antes de ` -- ` es la directiva y lo de
// después, el motivo. `eslint-disable` y `eslint-enable` sólo valen en comentarios de bloque.
fn parse(comment: &Comment) -> Option<(&'static str, Vec<String>, String)> {
    let justification = JUSTIFICATION.as_ref()?;
    let (directive, reason) = match justification.find(&comment.value) {
        Some(found) => (
            comment.value.get(..found.start())?.trim(),
            comment.value.get(found.end()..)?.trim(),
        ),
        None => (comment.value.trim(), ""),
    };
    let name = DIRECTIVE.as_ref()?.captures(directive)?.get(1)?;
    let kind = [CONFIGURE, DISABLE, ENABLE, DISABLE_LINE, DISABLE_NEXT_LINE]
        .into_iter()
        .find(|kind| *kind == name.as_str())?;
    let line_supported = matches!(kind, DISABLE_LINE | DISABLE_NEXT_LINE);
    if comment.single_line && !line_supported {
        return None;
    }
    let rest = directive.get(name.end()..)?;
    if kind == CONFIGURE {
        let rules = TURNED_OFF
            .as_ref()?
            .captures_iter(rest)
            .filter_map(|captures| captures.name("rule"))
            .map(|rule| kerbo_rule_name(rule.as_str()))
            .collect();
        return Some((kind, rules, reason.to_owned()));
    }
    let rules = rest
        .split(RULE_SEPARATOR)
        .map(str::trim)
        .filter(|rule| !rule.is_empty())
        .map(kerbo_rule_name)
        .collect();
    Some((kind, rules, reason.to_owned()))
}

// El nombre de la regla en kerbo: las de typescript-eslint van sin el prefijo del plugin.
fn kerbo_rule_name(rule: &str) -> String {
    UNIVERSAL_RULES
        .iter()
        .find(|(eslint, _)| *eslint == rule)
        .map_or_else(
            || rule.strip_prefix(TYPESCRIPT_PLUGIN).unwrap_or(rule).to_owned(),
            |(_, kerbo)| (*kerbo).to_owned(),
        )
}

fn comments(source: &str, language: Language) -> Vec<Comment> {
    let Some(grammar) = language.grammar() else {
        return Vec::new();
    };
    let mut parser = Parser::new();
    if parser.set_language(&grammar).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    collect_comments(tree.root_node(), source, &mut found);
    found.sort_by_key(|comment| comment.start);
    found
}

fn collect_comments(node: Node<'_>, source: &str, found: &mut Vec<Comment>) {
    if node.kind() == COMMENT {
        let text = source.get(node.byte_range()).unwrap_or_default();
        if let Some(value) = text.strip_prefix(LINE_PREFIX) {
            found.push(comment(true, value, node));
        } else if let Some(value) = text
            .strip_prefix(BLOCK_PREFIX)
            .and_then(|rest| rest.strip_suffix(BLOCK_SUFFIX))
        {
            found.push(comment(false, value, node));
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_comments(child, source, found);
    }
}

fn comment(single_line: bool, value: &str, node: Node<'_>) -> Comment {
    Comment {
        single_line,
        value: value.to_owned(),
        text: value.trim().to_owned(),
        start: node.start_position(),
        end: node.end_position(),
    }
}
