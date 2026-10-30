use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings};
use crate::shell::rules::command_checks::word_shapes::is_glob;
use crate::shell::syntax::{
    COMMAND, COMMAND_SUBSTITUTION, CONCATENATION, DECLARATION, EXPANSION, EXPANSIONS, RAW_STRING, REDIRECTED,
    STRING, UNSET, WORD, ancestors, children, first_token, named_children, text,
};
use crate::shell::words::is_brace_expansion;

const DOLLAR_PARENTHESIS: &str = "$(";
const HARMLESS_LITERALS: &[&str] = &["=", ":", "/"];
const COMMAND_KINDS: &[&str] = &[COMMAND, REDIRECTED, DECLARATION, UNSET];

// `checkInexplicablyUnquoted` de ShellCheck sobre las partes de una palabra.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() != CONCATENATION {
        return;
    }
    let source = context.source;
    let pieces = children(node);
    for window in pieces.windows(2) {
        if let [quoted, outside] = window
            && quoted.kind() == RAW_STRING
            && outside.kind() == WORD
            && text(*outside, source).chars().all(char::is_alphanumeric)
            && !text(*outside, source).is_empty()
        {
            findings.report(ShellRule::Sc2026, *outside, text(*outside, source));
        }
    }
    // Dentro de `{"a","b"}` las partes son de la expansión de llaves, no de la palabra.
    if is_brace_expansion(node, source) {
        return;
    }
    for window in pieces.windows(3) {
        let [before, trapped, after] = window else {
            continue;
        };
        if before.kind() != STRING || after.kind() != STRING {
            continue;
        }
        check_trapped((*before, *after), *trapped, source, findings);
    }
}

// `before` y `after` son las dos partes entre comillas dobles que rodean a `trapped`.
fn check_trapped(
    (before, after): (Node<'_>, Node<'_>),
    trapped: Node<'_>,
    source: &[u8],
    findings: &mut Findings,
) {
    let is_dollar_substitution =
        trapped.kind() == COMMAND_SUBSTITUTION && first_token(trapped, source) == DOLLAR_PARENTHESIS;
    if is_dollar_substitution || EXPANSIONS.contains(&trapped.kind()) {
        findings.report(ShellRule::Sc2027, trapped, text(trapped, source));
        return;
    }
    if trapped.kind() != WORD {
        return;
    }
    let literal = text(trapped, source);
    let deliberate = quotes_single_thing(before) && quotes_single_thing(after);
    // Para ShellCheck `/*` entre dos partes con comillas son dos tokens (un literal y un glob): el
    // patrón no calza.
    if !deliberate
        && !HARMLESS_LITERALS.contains(&literal)
        && !is_special(trapped)
        && !is_glob(trapped, source)
    {
        findings.report(ShellRule::Sc2140, trapped, literal);
    }
}

// Las comillas de `"$a"_"$b"` rodean una sola expansión cada una: son a propósito.
fn quotes_single_thing(string: Node<'_>) -> bool {
    let [only] = named_children(string)[..] else {
        return false;
    };
    only.kind() == COMMAND_SUBSTITUTION || EXPANSIONS.contains(&only.kind())
}

// Dentro de `${x+"a" "b"}` las comillas se leen distinto; dentro de un comando, no.
fn is_special(trapped: Node<'_>) -> bool {
    for ancestor in ancestors(trapped) {
        if COMMAND_KINDS.contains(&ancestor.kind()) {
            return false;
        }
        if ancestor.kind() == EXPANSION {
            return true;
        }
    }
    false
}
