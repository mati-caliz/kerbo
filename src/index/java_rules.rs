use std::collections::{HashMap, HashSet};

use super::java::{CallSite, Declaration, JavaSummary, lint_at};
use crate::lint::LintFinding;
use crate::lint::java_calls::{Arity, Body, Returns, Visibility};
use crate::rules::JavaRule;

const STRING_QUOTE: char = '"';
const QUALIFIER_SEPARATOR: char = '.';

type Key<'summary> = (bool, &'summary str, usize);

// Desde dónde se ve una constante: su visibilidad, su paquete y si está en los tests.
type Reach<'summary> = (Visibility, &'summary str, bool);

// Lo que se sabe del proyecto entero: cuántas veces se declara cada método o constructor (por
// nombre y cantidad de parámetros), quién lo llama y qué nombres se usan como referencia.
struct Project<'summary> {
    declared: HashMap<Key<'summary>, usize>,
    calls: HashMap<Key<'summary>, Vec<&'summary CallSite>>,
    referenced: HashSet<&'summary str>,
    overriding: HashSet<Key<'summary>>,
    constants: HashMap<(&'summary str, &'summary str), Vec<Reach<'summary>>>,
}

// Un archivo del índice: su resumen y si es de tests (el código de producción no ve lo de los
// tests).
pub struct IndexedSummary<'summary> {
    pub summary: &'summary JavaSummary,
    pub in_tests: bool,
}

const fn key_of(declaration: &Declaration) -> Key<'_> {
    (
        declaration.is_constructor,
        declaration.name.as_str(),
        declaration.parameters.len(),
    )
}

impl<'summary> Project<'summary> {
    fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut project = Self {
            declared: HashMap::new(),
            calls: HashMap::new(),
            referenced: HashSet::new(),
            overriding: HashSet::new(),
            constants: HashMap::new(),
        };
        for file in files {
            project.add(file);
        }
        project
    }

    fn add(&mut self, file: &IndexedSummary<'summary>) {
        let summary = file.summary;
        for constant in &summary.constants {
            self.constants
                .entry((constant.owner.as_str(), constant.name.as_str()))
                .or_default()
                .push((constant.visibility, summary.package.as_str(), file.in_tests));
        }
        for declaration in &summary.declarations {
            *self.declared.entry(key_of(declaration)).or_default() += 1;
        }
        self.overriding.extend(
            summary
                .declarations
                .iter()
                .filter(|declaration| declaration.overrides)
                .map(key_of),
        );
        for call in &summary.calls {
            self.calls
                .entry((call.is_constructor, call.name.as_str(), call.arguments.len()))
                .or_default()
                .push(call);
        }
        self.referenced
            .extend(summary.referenced.iter().map(String::as_str));
    }

    // Sin tipos no se sabe a cuál de dos métodos con el mismo nombre y aridad va una llamada, ni si
    // una referencia a método lo usa con otros valores: en la duda, no se razona sobre él.
    fn is_traceable(&self, declaration: &'summary Declaration) -> bool {
        self.declared.get(&key_of(declaration)) == Some(&1)
            && !declaration.overrides
            && (declaration.is_constructor || !self.referenced.contains(declaration.name.as_str()))
    }

    // Si ningún método con el mismo nombre y aridad dice @Override, ninguno redefine a éste.
    fn is_overridden(&self, declaration: &'summary Declaration) -> bool {
        self.overriding.contains(&key_of(declaration))
    }

    // IntelliJ marca un valor sólo si se puede llevar adentro del método: un literal, una constante
    // de una clase del mismo archivo, una pública (o de afuera del proyecto), o una de paquete del
    // mismo paquete que no esté en los tests si el método no lo está.
    fn is_movable_into(&self, value: &str, file: &IndexedSummary<'_>) -> bool {
        let Some((owner, name)) = value
            .split_once(QUALIFIER_SEPARATOR)
            .filter(|(owner, _)| owner.starts_with(|first: char| first.is_ascii_uppercase()))
        else {
            return true;
        };
        if file.summary.types.iter().any(|known| known == owner) {
            return true;
        }
        self.constants.get(&(owner, name)).is_none_or(|declared| {
            declared
                .iter()
                .any(|(visibility, package, in_tests)| match visibility {
                    Visibility::Public => true,
                    Visibility::Private => false,
                    Visibility::Package | Visibility::Protected => {
                        *package == file.summary.package && (file.in_tests || !in_tests)
                    }
                })
        })
    }

    fn calls_to(&self, declaration: &'summary Declaration) -> &[&'summary CallSite] {
        self.calls.get(&key_of(declaration)).map_or(&[], Vec::as_slice)
    }
}

// Las inspecciones de IntelliJ que miran el proyecto entero, por archivo. Sólo valen si el
// análisis ve todo el proyecto: una llamada que falta cambia la respuesta.
pub fn project_lints(files: &[IndexedSummary<'_>]) -> Vec<Vec<LintFinding>> {
    let project = Project::new(files);
    files
        .iter()
        .map(|file| {
            let summary = file.summary;
            summary
                .declarations
                .iter()
                .flat_map(|declaration| {
                    let mut found = same_parameter_values(&project, file, declaration);
                    found.extend(unused_return_value(&project, declaration));
                    found.extend(same_return_value(&project, summary, declaration));
                    found
                })
                .collect()
        })
        .collect()
}

// SameParameterValue para lo de paquete y lo protegido (lo privado se mira por archivo, y IntelliJ
// no marca lo público).
fn same_parameter_values<'summary>(
    project: &Project<'summary>,
    file: &IndexedSummary<'_>,
    declaration: &'summary Declaration,
) -> Vec<LintFinding> {
    let visible_in_project = matches!(
        declaration.visibility,
        Visibility::Package | Visibility::Protected
    );
    if !visible_in_project
        || declaration.body == Body::Abstract
        || declaration.arity == Arity::Variadic
        || !project.is_traceable(declaration)
    {
        return Vec::new();
    }
    let calls = project.calls_to(declaration);
    declaration
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            let value = shared_argument(calls, index).filter(|value| project.is_movable_into(value, file))?;
            Some(lint_at(
                JavaRule::SameParameterValue,
                &parameter.position,
                format!("{} = {value}", parameter.name),
            ))
        })
        .collect()
}

fn shared_argument(calls: &[&CallSite], index: usize) -> Option<String> {
    let mut values = calls
        .iter()
        .map(|call| call.arguments.get(index).cloned().flatten());
    let first = values.next()??;
    values
        .all(|value| value.as_deref() == Some(first.as_str()))
        .then_some(first)
}

// UnusedReturnValue: un método que devuelve algo, al que todas las llamadas del proyecto le tiran
// el resultado.
fn unused_return_value<'summary>(
    project: &Project<'summary>,
    declaration: &'summary Declaration,
) -> Option<LintFinding> {
    if declaration.is_constructor
        || declaration.returns == Returns::Nothing
        || !project.is_traceable(declaration)
    {
        return None;
    }
    let calls = project.calls_to(declaration);
    (!calls.is_empty() && calls.iter().all(|call| !call.result_used)).then(|| {
        lint_at(
            JavaRule::UnusedReturnValue,
            &declaration.position,
            declaration.name.clone(),
        )
    })
}

// SameReturnValue: todos los `return` de un método devuelven la misma constante. Para lo privado
// alcanza con que no haya otro igual en su clase de nivel superior.
fn same_return_value<'summary>(
    project: &Project<'summary>,
    summary: &JavaSummary,
    declaration: &'summary Declaration,
) -> Option<LintFinding> {
    let Returns::Always(value) = &declaration.returns else {
        return None;
    };
    if declaration.body == Body::Abstract || declaration.overrides {
        return None;
    }
    let unique = if declaration.visibility == Visibility::Private {
        summary
            .declarations
            .iter()
            .filter(|other| other.scope == declaration.scope && key_of(other) == key_of(declaration))
            .count()
            == 1
    } else {
        !project.is_overridden(declaration)
    };
    unique.then(|| {
        lint_at(
            JavaRule::SameReturnValue,
            &declaration.position,
            format!("{}() = {}", declaration.name, shown(value)),
        )
    })
}

// IntelliJ muestra el texto de un literal de cadena sin las comillas.
fn shown(value: &str) -> &str {
    value
        .strip_prefix(STRING_QUOTE)
        .and_then(|inner| inner.strip_suffix(STRING_QUOTE))
        .unwrap_or(value)
}
