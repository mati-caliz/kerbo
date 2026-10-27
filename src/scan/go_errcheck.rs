use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::go_types::{CallFact, FileFacts, ModuleFacts, module_facts};
use crate::i18n::{Message, Problem};
use crate::lint::LintKind;
use crate::lint::go::module_root;
use crate::lint::go::platform::GoPlatform;
use crate::rules::{Diagnostic, GoRule, RuleId};
use crate::suppressions::Suppressed;

use super::ScanFailure;

const ARGUMENT_OPEN: &str = "(";
const ARGUMENT_CLOSE: &str = ")";
// `DefaultExcludedSymbols` de errcheck v1.30.0, que golangci-lint usa salvo que se apaguen.
const EXCLUDED_SYMBOLS: &[&str] = &[
    "(*bytes.Buffer).Write",
    "(*bytes.Buffer).WriteByte",
    "(*bytes.Buffer).WriteRune",
    "(*bytes.Buffer).WriteString",
    "crypto/rand.Read",
    "errors.AsType",
    "fmt.Print",
    "fmt.Printf",
    "fmt.Println",
    "fmt.Fprint(*bytes.Buffer)",
    "fmt.Fprintf(*bytes.Buffer)",
    "fmt.Fprintln(*bytes.Buffer)",
    "fmt.Fprint(*strings.Builder)",
    "fmt.Fprintf(*strings.Builder)",
    "fmt.Fprintln(*strings.Builder)",
    "fmt.Fprint(os.Stderr)",
    "fmt.Fprintf(os.Stderr)",
    "fmt.Fprintln(os.Stderr)",
    "(*io.PipeReader).CloseWithError",
    "(*io.PipeWriter).CloseWithError",
    "math/rand.Read",
    "(*math/rand.Rand).Read",
    "(*strings.Builder).Write",
    "(*strings.Builder).WriteByte",
    "(*strings.Builder).WriteRune",
    "(*strings.Builder).WriteString",
    "(hash.Hash).Write",
    "(*crypto/sha3.SHA3).Write",
    "(*crypto/sha3.SHAKE).Read",
    "(*crypto/sha3.SHAKE).Write",
    "(*hash/maphash.Hash).Write",
    "(*hash/maphash.Hash).WriteByte",
    "(*hash/maphash.Hash).WriteString",
];

const ERRCHECK: RuleId = RuleId::Go(GoRule::Errcheck);

// Los candidatos de errcheck de cada archivo se confirman con los tipos de su módulo: quedan los
// que de verdad tiran un `error` y no están excluidos, con el nombre que muestra golangci-lint.
pub fn confirm_unchecked_errors(
    diagnostics: &mut Vec<Diagnostic>,
    suppressed: &mut Vec<Suppressed>,
    platform: &GoPlatform,
) -> Vec<ScanFailure> {
    let roots: HashMap<PathBuf, PathBuf> = diagnostics
        .iter()
        .chain(suppressed.iter().map(|entry| &entry.diagnostic))
        .filter(|diagnostic| diagnostic.rule == ERRCHECK)
        .filter_map(|diagnostic| {
            let root = module_root(&diagnostic.absolute_path)?;
            Some((diagnostic.absolute_path.clone(), root))
        })
        .collect();
    let mut failures = Vec::new();
    let modules: HashMap<&Path, ModuleFacts> = roots
        .values()
        .collect::<HashSet<_>>()
        .into_iter()
        .filter_map(|root| match module_facts(root, platform) {
            Ok(facts) => Some((root.as_path(), facts)),
            Err(detail) => {
                failures.push(ScanFailure {
                    problem: Problem::GoTypesUnavailable {
                        module: root.display().to_string(),
                        detail,
                    },
                });
                None
            }
        })
        .collect();
    keep_confirmed(diagnostics, suppressed, &roots, &modules);
    failures
}

// Sin el módulo en disco (un texto del editor, un arreglo a medio aplicar) no hay tipos con qué
// confirmar los candidatos.
pub fn drop_unconfirmed(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.retain(|diagnostic| diagnostic.rule != ERRCHECK);
}

fn keep_confirmed(
    diagnostics: &mut Vec<Diagnostic>,
    suppressed: &mut Vec<Suppressed>,
    roots: &HashMap<PathBuf, PathBuf>,
    modules: &HashMap<&Path, ModuleFacts>,
) {
    let confirm = |diagnostic: &mut Diagnostic| -> bool {
        if diagnostic.rule != ERRCHECK {
            return true;
        }
        let facts = roots
            .get(&diagnostic.absolute_path)
            .and_then(|root| modules.get(root.as_path()))
            .and_then(|module| module.file(&diagnostic.absolute_path));
        let Some(subject) =
            facts.and_then(|facts| unchecked_subject(facts, diagnostic.line, diagnostic.column))
        else {
            return false;
        };
        diagnostic.message = Message::Lint {
            kind: LintKind::Go(GoRule::Errcheck),
            subject,
        };
        true
    };
    diagnostics.retain_mut(|diagnostic| confirm(diagnostic));
    suppressed.retain_mut(|entry| confirm(&mut entry.diagnostic));
}

// El nombre del aviso si en esa posición hay un error sin revisar; sin nombre, vacío.
fn unchecked_subject(facts: &FileFacts, line: usize, column: usize) -> Option<String> {
    let at = |fact_line: usize, fact_column: usize| (fact_line, fact_column) == (line, column);
    if let Some(call) = facts
        .calls
        .iter()
        .flatten()
        .find(|call| at(call.line, call.column))
    {
        return (!excluded(call)).then(|| code(call));
    }
    if facts
        .receives
        .iter()
        .flatten()
        .any(|receive| at(receive.line, receive.column))
    {
        return Some(String::new());
    }
    let range = facts
        .ranges
        .iter()
        .flatten()
        .find(|range| at(range.line, range.column))?;
    range.call.as_ref().map_or_else(
        || Some(String::new()),
        |call| (!excluded(call)).then(|| code(call)),
    )
}

fn code(call: &CallFact) -> String {
    if call.function.is_empty() {
        return String::new();
    }
    if call.selector.is_empty() {
        call.function.clone()
    } else {
        call.selector.clone()
    }
}

// `excludeCall` e `isErrorsAsType` de errcheck.
fn excluded(call: &CallFact) -> bool {
    call.errors_as_type
        || call.names.iter().flatten().any(|name| {
            EXCLUDED_SYMBOLS.contains(&name.as_str())
                || (!call.argument.is_empty()
                    && EXCLUDED_SYMBOLS.contains(
                        &[
                            name.as_str(),
                            ARGUMENT_OPEN,
                            call.argument.as_str(),
                            ARGUMENT_CLOSE,
                        ]
                        .concat()
                        .as_str(),
                    ))
        })
}

#[cfg(test)]
mod tests;
