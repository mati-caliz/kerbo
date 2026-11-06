use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::parameters::reference;
use crate::shell::rules::structure::cases::{Pattern, patterns};
use crate::shell::syntax::{
    BODY_FIELD, CASE, CASE_ITEM, COMMAND, EXPANSIONS, REDIRECTED, VALUE_FIELD, WHILE, ancestors, children,
    named_children, text,
};
use crate::shell::variables::{EventKind, flow};
use crate::shell::word_pieces::{literal_string, pieces};
use crate::shell::words::braced_content;

const GETOPTS: &str = "getopts";
const WHILE_KEYWORD: &str = "while";
const OPTION_ARGUMENT_MARK: char = ':';
const ANY_PATTERNS: &[&str] = &["*", "?"];
const IGNORED_PATTERNS: &[&str] = &["*", ":", "?"];

// SC2213, SC2214 y SC2220: `checkWhileGetoptsCase`.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() != COMMAND {
        return;
    }
    let source = context.source;
    let Some(command) = Command::of(node).filter(|command| command.is_exactly(GETOPTS, source)) else {
        return;
    };
    let [_, options_word, name_word, ..] = command.words[..] else {
        return;
    };
    let (Some(options), Some(variable)) = (
        literal_string(&pieces(options_word, source)),
        literal_string(&pieces(name_word, source)),
    ) else {
        return;
    };
    let Some(loop_node) = ancestors(node).find(|ancestor| ancestor.kind() == WHILE) else {
        return;
    };
    let is_while = children(loop_node)
        .first()
        .is_some_and(|keyword| text(*keyword, source) == WHILE_KEYWORD);
    let Some(body) = loop_node.child_by_field_name(BODY_FIELD).filter(|_| is_while) else {
        return;
    };
    let Some(case) = named_children(body).into_iter().find_map(case_of) else {
        return;
    };
    if !switches_on(case, &variable, source) || modifies(body, &variable, context) {
        return;
    }
    let requested: BTreeSet<String> = options
        .chars()
        .filter(|option| *option != OPTION_ARGUMENT_MARK)
        .map(String::from)
        .collect();
    check_case(case, &requested, source, findings);
}

fn case_of(statement: Node<'_>) -> Option<Node<'_>> {
    match statement.kind() {
        CASE => Some(statement),
        REDIRECTED => statement
            .child_by_field_name(BODY_FIELD)
            .filter(|body| body.kind() == CASE),
        _ => None,
    }
}

fn switches_on(case: Node<'_>, variable: &str, source: &[u8]) -> bool {
    case.child_by_field_name(VALUE_FIELD).is_some_and(|value| {
        let inner = match named_children(value)[..] {
            [only] if !EXPANSIONS.contains(&value.kind()) => only,
            _ => value,
        };
        EXPANSIONS.contains(&inner.kind()) && {
            let braced = braced_content(inner, source);
            braced == variable && reference(&braced) == braced
        }
    })
}

fn modifies(body: Node<'_>, variable: &str, context: &Context<'_, '_>) -> bool {
    flow(context).events.iter().any(|event| {
        event.kind == EventKind::Assignment
            && event.name == variable
            && event.position.byte >= body.start_byte()
            && event.position.byte < body.end_byte()
    })
}

// Para ShellCheck, un patrón con un glob no tiene texto: cuenta como desconocido y apaga SC2213 y SC2220.
fn pattern_key(pattern: &Pattern<'_>) -> Option<String> {
    literal_string(&pattern.pieces)
}

fn check_case(case: Node<'_>, requested: &BTreeSet<String>, source: &[u8], findings: &mut Findings) {
    let all_patterns: Vec<Pattern<'_>> = named_children(case)
        .into_iter()
        .filter(|child| child.kind() == CASE_ITEM)
        .flat_map(|item| patterns(item, source))
        .collect();
    let mut handled: BTreeMap<Option<String>, Node<'_>> = BTreeMap::new();
    for pattern in &all_patterns {
        handled.insert(pattern_key(pattern), pattern.first);
    }
    if !handled.contains_key(&None) {
        for option in requested {
            if !handled.contains_key(&Some(option.clone())) {
                findings.report(ShellRule::Sc2213, case, option);
            }
        }
        let handles_any = ANY_PATTERNS
            .iter()
            .any(|pattern| handled.contains_key(&Some((*pattern).to_owned())));
        if !handles_any {
            findings.report(ShellRule::Sc2220, case, GETOPTS);
        }
    }
    for (key, first) in &handled {
        if let Some(key) = key
            && !requested.contains(key)
            && !IGNORED_PATTERNS.contains(&key.as_str())
        {
            findings.report(ShellRule::Sc2214, *first, key);
        }
    }
}
