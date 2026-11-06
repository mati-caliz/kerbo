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
pub use direct::{DirectFile, direct_files};

mod direct;

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

const TYPED_RULES: [GoRule; 3] = [GoRule::Errcheck, GoRule::Qf1008, GoRule::Qf1012];

fn typed_rule(rule: RuleId) -> Option<GoRule> {
    if let RuleId::Go(go_rule) = rule
        && TYPED_RULES.contains(&go_rule)
    {
        return Some(go_rule);
    }
    None
}

// Los candidatos de las reglas que piden tipos se confirman con los de su módulo: quedan los que
// los tipos sostienen (un `error` que de verdad se tira y no está excluido, un campo embebido que
// se puede omitir, un io.Writer), con el nombre que muestra golangci-lint.
pub fn confirm_typed_findings(
    diagnostics: &mut Vec<Diagnostic>,
    suppressed: &mut Vec<Suppressed>,
    direct_files: &[DirectFile<'_>],
    platform: &GoPlatform,
) -> Vec<ScanFailure> {
    let roots: HashMap<PathBuf, PathBuf> = diagnostics
        .iter()
        .chain(suppressed.iter().map(|entry| &entry.diagnostic))
        .filter(|diagnostic| typed_rule(diagnostic.rule).is_some())
        .map(|diagnostic| &diagnostic.absolute_path)
        .chain(direct_files.iter().map(|file| &file.identity.absolute))
        .filter_map(|path| Some((path.clone(), module_root(path)?)))
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
    let confirmation = Confirmation {
        roots: &roots,
        modules: &modules,
        extra_excluded: platform.errcheck_exclude_functions(),
    };
    keep_confirmed(diagnostics, suppressed, &confirmation);
    let (direct, direct_suppressed) = direct::direct_reports(direct_files, &roots, &modules);
    diagnostics.extend(direct);
    suppressed.extend(direct_suppressed);
    failures
}

// Sin el módulo en disco (un texto del editor, un arreglo a medio aplicar) no hay tipos con qué
// confirmar los candidatos.
pub fn drop_unconfirmed(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.retain(|diagnostic| typed_rule(diagnostic.rule).is_none());
}

struct Confirmation<'run> {
    roots: &'run HashMap<PathBuf, PathBuf>,
    modules: &'run HashMap<&'run Path, ModuleFacts>,
    extra_excluded: &'run [String],
}

fn keep_confirmed(
    diagnostics: &mut Vec<Diagnostic>,
    suppressed: &mut Vec<Suppressed>,
    confirmation: &Confirmation<'_>,
) {
    let confirm = |diagnostic: &mut Diagnostic| -> bool {
        let Some(rule) = typed_rule(diagnostic.rule) else {
            return true;
        };
        let facts = confirmation
            .roots
            .get(&diagnostic.absolute_path)
            .and_then(|root| confirmation.modules.get(root.as_path()))
            .and_then(|module| module.file(&diagnostic.absolute_path));
        let position = (diagnostic.line, diagnostic.column);
        let confirmed = facts.and_then(|facts| {
            confirmed_subject(
                (rule, &diagnostic.message),
                facts,
                position,
                confirmation.extra_excluded,
            )
        });
        let Some(subject) = confirmed else {
            return false;
        };
        diagnostic.message = Message::Lint {
            kind: LintKind::Go(rule),
            subject,
        };
        true
    };
    diagnostics.retain_mut(|diagnostic| confirm(diagnostic));
    suppressed.retain_mut(|entry| confirm(&mut entry.diagnostic));
}

// Sólo errcheck, QF1012 y QF1008 tienen candidatos por archivo; las demás reglas de tipos son directas.
fn confirmed_subject(
    (rule, message): (GoRule, &Message),
    facts: &FileFacts,
    position: (usize, usize),
    extra_excluded: &[String],
) -> Option<String> {
    if rule == GoRule::Errcheck {
        return unchecked_subject(facts, position, extra_excluded);
    }
    if rule == GoRule::Qf1012 {
        return fprint_replacement(facts, position);
    }
    if rule == GoRule::Qf1008 {
        return removable_embedded_field(facts, position, message);
    }
    None
}

// QF1008: el campo que se puede sacar sigue siendo el sujeto del candidato.
fn removable_embedded_field(
    facts: &FileFacts,
    position: (usize, usize),
    message: &Message,
) -> Option<String> {
    let Message::Lint { subject, .. } = message else {
        return None;
    };
    facts
        .embedded
        .iter()
        .flatten()
        .any(|fact| (fact.line, fact.column) == position)
        .then(|| subject.clone())
}

// QF1012: la función de fmt que reemplaza a la escritura.
fn fprint_replacement(facts: &FileFacts, position: (usize, usize)) -> Option<String> {
    facts
        .fprints
        .iter()
        .flatten()
        .find(|fact| (fact.line, fact.column) == position)
        .map(|fact| fact.replacement.clone())
}

// El nombre del aviso si en esa posición hay un error sin revisar; sin nombre, vacío.
fn unchecked_subject(
    facts: &FileFacts,
    position: (usize, usize),
    extra_excluded: &[String],
) -> Option<String> {
    let at = |fact_line: usize, fact_column: usize| (fact_line, fact_column) == position;
    if let Some(call) = facts
        .calls
        .iter()
        .flatten()
        .find(|call| at(call.line, call.column))
    {
        return (!excluded(call, extra_excluded)).then(|| code(call));
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
        |call| (!excluded(call, extra_excluded)).then(|| code(call)),
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
fn excluded(call: &CallFact, extra_excluded: &[String]) -> bool {
    let is_excluded = |symbol: &str| {
        EXCLUDED_SYMBOLS.contains(&symbol) || extra_excluded.iter().any(|extra| extra == symbol)
    };
    call.errors_as_type
        || call.names.iter().flatten().any(|name| {
            is_excluded(name)
                || (!call.argument.is_empty()
                    && is_excluded(
                        &[
                            name.as_str(),
                            ARGUMENT_OPEN,
                            call.argument.as_str(),
                            ARGUMENT_CLOSE,
                        ]
                        .concat(),
                    ))
        })
}

#[cfg(test)]
mod tests;
