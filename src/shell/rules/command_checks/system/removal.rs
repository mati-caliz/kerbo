use tree_sitter::Node;

use super::super::Invocation;
use super::potential_path;
use crate::rules::ShellRule;
use crate::shell::commands::flags_of;
use crate::shell::context::Findings;
use crate::shell::syntax::{WORD, text};
use crate::shell::words::{literal, parts};

const RM: &str = "rm";
const RECURSIVE_FLAGS: &[&str] = &["r", "R", "recursive"];
const SYSTEM_DIRECTORIES: &[&str] = &[
    "",
    "/bin",
    "/etc",
    "/home",
    "/mnt",
    "/usr",
    "/usr/share",
    "/usr/local",
    "/var",
    "/lib",
    "/dev",
    "/media",
    "/boot",
    "/lib64",
    "/usr/bin",
];
const SUFFIXES: &[&str] = &["", "/", "/*", "/*/*"];
const ROOT: &str = "/";
const SEPARATOR: char = '/';
const STAR: char = '*';
const OPEN_BRACE: char = '{';
const CLOSE_BRACE: char = '}';
const COMMA: char = ',';
const MAXIMUM_EXPANSIONS: usize = 1000;

// `rm -r` sobre `/`, `/usr`, `/etc/*` y los demás directorios del sistema: SC2114 si está escrito
// así, SC2115 si sale de una variable que puede estar vacía.
pub fn check(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    if !invocation.has_basename(RM) {
        return;
    }
    let recursive = flags_of(invocation.arguments(), source)
        .iter()
        .any(|(_, flag)| RECURSIVE_FLAGS.contains(&flag.as_str()));
    if !recursive {
        return;
    }
    for argument in invocation.arguments() {
        check_path(*argument, source, findings);
    }
}

fn check_path(word: Node<'_>, source: &[u8], findings: &mut Findings) {
    if let Some(path) = literal(word, source) {
        if is_system_directory(&path) {
            findings.report(ShellRule::Sc2114, word, &path);
        }
        return;
    }
    let Some(path) = potential_path(word, source) else {
        return;
    };
    let rule = if parts(word).iter().all(|part| part.kind() == WORD) {
        ShellRule::Sc2114
    } else {
        ShellRule::Sc2115
    };
    for _ in brace_expand(&path)
        .iter()
        .filter(|candidate| is_system_directory(candidate))
    {
        findings.report(rule, word, text(word, source));
    }
}

// `braceExpand`: cada palabra que sale de las llaves con coma, incluidas las anidadas.
fn brace_expand(path: &str) -> Vec<String> {
    let Some((open, close)) = first_brace_list(path) else {
        return vec![path.to_owned()];
    };
    let prefix = path.get(..open).unwrap_or_default();
    let suffix = path.get(close.saturating_add(1)..).unwrap_or_default();
    let inside = path.get(open.saturating_add(1)..close).unwrap_or_default();
    let mut expanded = Vec::new();
    for alternative in split_alternatives(inside) {
        for rest in brace_expand(&format!("{alternative}{suffix}")) {
            if expanded.len() < MAXIMUM_EXPANSIONS {
                expanded.push(format!("{prefix}{rest}"));
            }
        }
    }
    expanded
}

fn first_brace_list(path: &str) -> Option<(usize, usize)> {
    path.char_indices()
        .filter(|(_, character)| *character == OPEN_BRACE)
        .find_map(|(open, _)| closing_of_list(path, open).map(|close| (open, close)))
}

// El `}` que cierra la llave abierta en `open`, si adentro hay una coma de su mismo nivel.
fn closing_of_list(path: &str, open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    let mut has_comma = false;
    for (offset, inner) in path.get(open..).unwrap_or_default().char_indices() {
        match inner {
            OPEN_BRACE => depth = depth.saturating_add(1),
            CLOSE_BRACE => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return has_comma.then_some(open.saturating_add(offset));
                }
            }
            COMMA if depth == 1 => has_comma = true,
            _ => {}
        }
    }
    None
}

fn split_alternatives(inside: &str) -> Vec<String> {
    let mut alternatives = vec![String::new()];
    let mut depth = 0_usize;
    for character in inside.chars() {
        match character {
            OPEN_BRACE => depth = depth.saturating_add(1),
            CLOSE_BRACE => depth = depth.saturating_sub(1),
            COMMA if depth == 0 => {
                alternatives.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(current) = alternatives.last_mut() {
            current.push(character);
        }
    }
    alternatives
}

fn is_system_directory(path: &str) -> bool {
    let normalized = collapse(&collapse(path, STAR), SEPARATOR);
    let fixed = if normalized == ROOT {
        normalized
    } else {
        normalized.trim_end_matches(SEPARATOR).to_owned()
    };
    SUFFIXES.iter().any(|suffix| {
        SYSTEM_DIRECTORIES
            .iter()
            .map(|directory| format!("{directory}{suffix}"))
            .any(|candidate| !candidate.is_empty() && candidate == fixed)
    })
}

// `skipRepeating`: deja uno solo de cada racha del carácter.
fn collapse(path: &str, repeated: char) -> String {
    let mut collapsed = String::new();
    for character in path.chars() {
        if character == repeated && collapsed.ends_with(repeated) {
            continue;
        }
        collapsed.push(character);
    }
    collapsed
}
