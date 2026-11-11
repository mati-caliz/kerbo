use super::java::{Declaration, JavaSummary, Position, lint_at};
use super::java_null_arguments::accepts;
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_calls::Arity;
use crate::lint::java_contract_calls::{CallTarget, ContractCall};
use crate::lint::java_contracts::{Clause, Constraint};
use crate::lint::java_receivers::{Receiver, simple_type_name};
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';
const WILDCARD_SUFFIX: &str = ".*";
const BOOLEAN_TYPES: &[&str] = &["boolean", "Boolean"];
const VALIDATION_CLASSES: &[&str] = &[
    "org.apache.commons.lang.Validate",
    "org.apache.commons.lang3.Validate",
    "org.springframework.util.Assert",
];
const VALIDATION_TRUE_CHECKS: &[&str] = &["isTrue", "state"];
const VALIDATION_NOT_NULL: &str = "notNull";
const JUNIT_PREFIXES: &[&str] = &["junit.framework.", "org.junit."];
const JUNIT_ASSERT_TESTNG: &str = "org.testng.AssertJUnit";
const TESTNG_PREFIX: &str = "org.testng.";
const JUPITER_PREFIX: &str = "org.junit.jupiter";
const ASSERT_INSTANCE_OF: &str = "assertInstanceOf";
const ASSERT_INSTANCE_OF_VALUE: usize = 1;
const TEST_ASSERTION_FAILURES: &[(&str, Constraint)] = &[
    ("assertTrue", Constraint::False),
    ("assumeTrue", Constraint::False),
    ("assertFalse", Constraint::True),
    ("assumeFalse", Constraint::True),
    ("assertNull", Constraint::NotNull),
    ("assertNotNull", Constraint::Null),
];

// Calca el "The call to '...' always fails, according to its method contracts" de DataFlowIssue:
// con lo que se sabe de los argumentos, las cláusulas `fail` del método se cumplen siempre.
pub fn failing_calls(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    in_tests: bool,
) -> Vec<LintFinding> {
    summary
        .contract_calls
        .iter()
        .filter(|call| !(in_tests && call.exception_expected))
        .filter(|call| always_fails(types, library, summary, call))
        .map(|call| {
            lint_at(
                JavaRule::FailingCall,
                &Position {
                    line: call.line,
                    column: call.column,
                },
                call.name.clone(),
            )
        })
        .collect()
}

fn always_fails(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    call: &ContractCall,
) -> bool {
    match &call.receiver {
        Receiver::Type(written) => match resolve(types, library, summary, written) {
            Resolved::Project(owner) => {
                let owners = if call.target == CallTarget::Constructor {
                    vec![owner]
                } else {
                    project_lineage(types, library, &owner)
                };
                fails_in_owners(types, summary, &owners, call)
            }
            Resolved::Library(class) => {
                hardcoded_clause(&class, call).is_some_and(|clause| fails_with(&[clause], &call.arguments))
            }
        },
        Receiver::Scope(enclosing) => {
            let owners: Vec<String> = enclosing
                .iter()
                .flat_map(|owner| project_lineage(types, library, owner))
                .collect();
            if call.target != CallTarget::ImportedMethod || declares(types, &owners, call) {
                return fails_in_owners(types, summary, &owners, call);
            }
            fails_through_static_import(types, summary, call)
        }
        Receiver::Unknown => false,
    }
}

fn declares(types: &ProjectTypes<'_>, owners: &[String], call: &ContractCall) -> bool {
    owners.iter().any(|owner| {
        types
            .declarations_of(owner)
            .any(|(_, declaration)| declaration.name == call.name)
    })
}

fn fails_through_static_import(types: &ProjectTypes<'_>, summary: &JavaSummary, call: &ContractCall) -> bool {
    let explicit_suffix = format!("{QUALIFIER_SEPARATOR}{}", call.name);
    let explicit = summary
        .static_imports
        .iter()
        .filter_map(|path| path.strip_suffix(explicit_suffix.as_str()));
    let wildcards = summary
        .static_imports
        .iter()
        .filter_map(|path| path.strip_suffix(WILDCARD_SUFFIX));
    explicit
        .chain(wildcards)
        .find_map(|class| imported_class_verdict(types, summary, class, call))
        .unwrap_or(false)
}

fn imported_class_verdict(
    types: &ProjectTypes<'_>,
    summary: &JavaSummary,
    class: &str,
    call: &ContractCall,
) -> Option<bool> {
    if let Some(clause) = hardcoded_clause(class, call) {
        return Some(fails_with(&[clause], &call.arguments));
    }
    let owner = simple_type_name(class);
    declares(types, std::slice::from_ref(&owner), call)
        .then(|| fails_in_owners(types, summary, &[owner], call))
}

fn qualified_name(types: &ProjectTypes<'_>, summary: &JavaSummary, owner: &str) -> Option<String> {
    let declaring = types.declaring_file(owner, summary)?;
    Some(if declaring.package.is_empty() {
        owner.to_owned()
    } else {
        format!("{}{QUALIFIER_SEPARATOR}{owner}", declaring.package)
    })
}

// IntelliJ no busca la sobrecarga exacta: si alguna de las que pueden recibir los argumentos no
// falla siempre, no avisa. Un método que falla con cualquier argumento queda en `_ -> fail`, que
// tampoco avisa.
fn fails_in_owners(
    types: &ProjectTypes<'_>,
    summary: &JavaSummary,
    owners: &[String],
    call: &ContractCall,
) -> bool {
    owners
        .iter()
        .find_map(|owner| verdict_in_owner(types, summary, owner, call))
        .unwrap_or(false)
}

fn verdict_in_owner(
    types: &ProjectTypes<'_>,
    summary: &JavaSummary,
    owner: &str,
    call: &ContractCall,
) -> Option<bool> {
    if let Some(clause) =
        qualified_name(types, summary, owner).and_then(|qualified| hardcoded_clause(&qualified, call))
    {
        return Some(fails_with(&[clause], &call.arguments));
    }
    let candidates: Vec<&Declaration> = types
        .declarations_of(owner)
        .map(|(_, declaration)| declaration)
        .filter(|declaration| receives(declaration, call))
        .collect();
    (!candidates.is_empty()).then(|| {
        candidates.iter().all(|declaration| {
            !declaration.fail_clauses.is_empty()
                && !fails_with(
                    &declaration.fail_clauses,
                    &vec![None; declaration.parameters.len()],
                )
                && fails_with(&declaration.fail_clauses, &parameter_facts(declaration, call))
        })
    })
}

fn receives(declaration: &Declaration, call: &ContractCall) -> bool {
    let last = declaration.parameters.len().saturating_sub(1);
    declaration.is_constructor == (call.target == CallTarget::Constructor)
        && declaration.name == call.name
        && accepts(declaration, call.arity)
        && call.arguments.iter().enumerate().all(|(index, fact)| {
            let Some(parameter) = declaration.parameters.get(index.min(last)) else {
                return fact.is_none();
            };
            match fact {
                Some(Constraint::Null) => parameter.accepts_null,
                Some(Constraint::True | Constraint::False) => {
                    BOOLEAN_TYPES.contains(&parameter.type_name.as_str())
                        || declaration.arity == Arity::Variadic
                }
                Some(Constraint::NotNull) | None => true,
            }
        })
}

// En el varargs el argumento cuenta sólo si es el único de ahí: si no, la llamada arma un arreglo,
// que nunca es nulo.
fn parameter_facts(declaration: &Declaration, call: &ContractCall) -> Vec<Option<Constraint>> {
    let count = declaration.parameters.len();
    (0..count)
        .map(|index| {
            let is_varargs = declaration.arity == Arity::Variadic && index + 1 == count;
            if is_varargs && call.arity != count {
                Some(Constraint::NotNull)
            } else {
                call.arguments.get(index).copied().flatten()
            }
        })
        .collect()
}

// Calca la búsqueda de IntelliJ: cada cláusula deja vivos los estados que la violan y, por cada
// condición que no se sabe, uno nuevo con la condición negada. Si no queda ninguno, falla siempre.
fn fails_with(clauses: &[Clause], facts: &[Option<Constraint>]) -> bool {
    let mut states = vec![facts.to_vec()];
    for clause in clauses {
        states = states
            .into_iter()
            .flat_map(|state| survivors(clause, state))
            .collect();
        if states.is_empty() {
            return true;
        }
    }
    false
}

fn survivors(clause: &Clause, state: Vec<Option<Constraint>>) -> Vec<Vec<Option<Constraint>>> {
    let constrained: Vec<(usize, Constraint)> = clause
        .iter()
        .enumerate()
        .filter_map(|(index, constraint)| constraint.map(|constraint| (index, constraint)))
        .collect();
    let known = |index: usize| state.get(index).copied().flatten();
    if constrained
        .iter()
        .any(|(index, constraint)| known(*index).is_some_and(|fact| violates(fact, *constraint)))
    {
        return vec![state];
    }
    constrained
        .iter()
        .filter(|(index, _)| known(*index).is_none())
        .map(|(index, constraint)| {
            let mut survivor = state.clone();
            if let Some(slot) = survivor.get_mut(*index) {
                *slot = Some(constraint.negate());
            }
            survivor
        })
        .collect()
}

const fn violates(fact: Constraint, constraint: Constraint) -> bool {
    matches!(
        (fact, constraint),
        (
            Constraint::Null,
            Constraint::NotNull | Constraint::True | Constraint::False
        ) | (Constraint::NotNull, Constraint::Null)
            | (Constraint::True, Constraint::False | Constraint::Null)
            | (Constraint::False, Constraint::True | Constraint::Null)
    )
}

// Los contratos que IntelliJ trae escritos a mano para las validaciones de commons-lang, Spring,
// JUnit y TestNG.
fn hardcoded_clause(class: &str, call: &ContractCall) -> Option<Clause> {
    if call.target == CallTarget::Constructor || call.arity == 0 {
        return None;
    }
    let (index, constraint) = if VALIDATION_CLASSES.contains(&class) {
        validation_failure(&call.name)?
    } else if class == JUNIT_ASSERT_TESTNG || JUNIT_PREFIXES.iter().any(|prefix| class.starts_with(prefix)) {
        if call.name == ASSERT_INSTANCE_OF {
            (ASSERT_INSTANCE_OF_VALUE, Constraint::Null)
        } else {
            let checked = if class.starts_with(JUPITER_PREFIX) {
                0
            } else {
                call.arity - 1
            };
            (checked, test_assertion_failure(&call.name)?)
        }
    } else if class.starts_with(TESTNG_PREFIX) {
        (0, test_assertion_failure(&call.name)?)
    } else {
        return None;
    };
    let mut clause = vec![None; call.arity];
    *clause.get_mut(index)? = Some(constraint);
    Some(clause)
}

fn validation_failure(name: &str) -> Option<(usize, Constraint)> {
    if VALIDATION_TRUE_CHECKS.contains(&name) {
        Some((0, Constraint::False))
    } else if name == VALIDATION_NOT_NULL {
        Some((0, Constraint::Null))
    } else {
        None
    }
}

fn test_assertion_failure(name: &str) -> Option<Constraint> {
    TEST_ASSERTION_FAILURES
        .iter()
        .find(|(assertion, _)| *assertion == name)
        .map(|(_, constraint)| *constraint)
}
