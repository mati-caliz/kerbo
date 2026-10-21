use std::collections::{HashMap, HashSet};

use super::java::{CallSite, Declaration, JavaSummary};
use crate::lint::java_calls::Visibility;
use crate::lint::java_receivers::Receiver;

const QUALIFIER_SEPARATOR: char = '.';

pub type Key<'summary> = (bool, &'summary str, usize);

// Desde dónde se ve una constante: su visibilidad, su paquete y si está en los tests.
type Reach<'summary> = (Visibility, &'summary str, bool);

// Un archivo del índice: su resumen y si es de tests (el código de producción no ve lo de los
// tests).
pub struct IndexedSummary<'summary> {
    pub summary: &'summary JavaSummary,
    pub in_tests: bool,
}

pub const fn key_of(declaration: &Declaration) -> Key<'_> {
    (
        declaration.is_constructor,
        declaration.name.as_str(),
        declaration.parameters.len(),
    )
}

// Lo que se sabe del proyecto entero: qué se declara y quién lo llama (por nombre y cantidad de
// parámetros), qué tipo hereda de cuál, qué nombres se usan como referencia o en un texto, y desde
// dónde se ve cada constante.
pub struct Project<'summary> {
    declared: HashMap<Key<'summary>, Vec<&'summary Declaration>>,
    calls: HashMap<Key<'summary>, Vec<&'summary CallSite>>,
    calls_by_name: HashMap<&'summary str, Vec<&'summary CallSite>>,
    ancestors: HashMap<&'summary str, HashSet<&'summary str>>,
    named: HashSet<&'summary str>,
    referenced: HashSet<&'summary str>,
    constants: HashMap<(&'summary str, &'summary str), Vec<Reach<'summary>>>,
}

impl<'summary> Project<'summary> {
    pub fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut project = Self {
            declared: HashMap::new(),
            calls: HashMap::new(),
            calls_by_name: HashMap::new(),
            ancestors: HashMap::new(),
            named: HashSet::new(),
            referenced: HashSet::new(),
            constants: HashMap::new(),
        };
        let mut parents: HashMap<&str, Vec<&str>> = HashMap::new();
        for file in files {
            project.add(file);
            for (name, supertypes) in &file.summary.hierarchy {
                parents
                    .entry(name.as_str())
                    .or_default()
                    .extend(supertypes.iter().map(String::as_str));
            }
        }
        project.ancestors = parents
            .keys()
            .map(|name| (*name, ancestors_of(name, &parents)))
            .collect();
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
            self.declared
                .entry(key_of(declaration))
                .or_default()
                .push(declaration);
        }
        for call in &summary.calls {
            self.calls
                .entry((call.is_constructor, call.name.as_str(), call.arguments.len()))
                .or_default()
                .push(call);
            if !call.is_constructor {
                self.calls_by_name
                    .entry(call.name.as_str())
                    .or_default()
                    .push(call);
            }
        }
        self.referenced
            .extend(summary.referenced.iter().map(String::as_str));
        self.named.extend(summary.referenced.iter().map(String::as_str));
        self.named.extend(summary.mentioned.iter().map(String::as_str));
    }

    fn is_ancestor(&self, ancestor: &str, of: &str) -> bool {
        ancestor == of
            || self
                .ancestors
                .get(of)
                .is_some_and(|ancestors| ancestors.contains(ancestor))
    }

    // Si una llamada puede ir a un método de `owner`: con el receptor desconocido (o una variable de
    // tipo genérico, `T item`), sí; con un tipo, si `owner` es él o uno de sus padres; desde
    // adentro de unas clases, si alguna de ellas es o hereda de `owner`.
    fn reaches(&self, call: &CallSite, owner: &str) -> bool {
        match &call.receiver {
            Receiver::Unknown => true,
            Receiver::Type(declared) => {
                self.is_ancestor(owner, declared)
                    || (!self.ancestors.contains_key(declared.as_str()) && is_type_variable(declared))
            }
            Receiver::Scope(enclosing) => enclosing.iter().any(|inner| self.is_ancestor(owner, inner)),
        }
    }

    fn are_related(&self, one: &str, other: &str) -> bool {
        self.is_ancestor(one, other) || self.is_ancestor(other, one)
    }

    // Otro método con el mismo nombre y aridad en la misma clase (una sobrecarga) o en una clase
    // emparentada (una redefinición): no se sabe a cuál va cada llamada.
    pub fn has_twin(&self, declaration: &'summary Declaration) -> bool {
        self.declared.get(&key_of(declaration)).is_some_and(|twins| {
            twins.iter().any(|twin| {
                !std::ptr::eq(*twin, declaration)
                    && self.are_related(&twin.owner.name, &declaration.owner.name)
            })
        })
    }

    // Si hay métodos con el mismo nombre y aridad en clases sin relación, las llamadas de receptor
    // desconocido pueden ser de cualquiera: hace falta al menos una que se sepa que va a éste.
    fn has_strangers(&self, declaration: &'summary Declaration) -> bool {
        self.declared
            .get(&key_of(declaration))
            .is_some_and(|twins| twins.len() > 1)
    }

    pub fn is_traceable(&self, declaration: &'summary Declaration) -> bool {
        !declaration.overrides
            && !self.has_twin(declaration)
            && (declaration.is_constructor || !self.referenced.contains(declaration.name.as_str()))
    }

    // Una redefinición con `@Override` en una clase que hereda de la de éste.
    pub fn is_overridden(&self, declaration: &'summary Declaration) -> bool {
        self.declared.get(&key_of(declaration)).is_some_and(|twins| {
            twins.iter().any(|twin| {
                twin.overrides
                    && !std::ptr::eq(*twin, declaration)
                    && self.is_ancestor(&declaration.owner.name, &twin.owner.name)
            })
        })
    }

    pub fn calls_to(&self, declaration: &'summary Declaration) -> Vec<&'summary CallSite> {
        let owner = declaration.owner.name.as_str();
        let calls: Vec<&CallSite> = self
            .calls
            .get(&key_of(declaration))
            .map(|calls| {
                calls
                    .iter()
                    .copied()
                    .filter(|call| declaration.is_constructor || self.reaches(call, owner))
                    .collect()
            })
            .unwrap_or_default();
        let resolved = calls
            .iter()
            .any(|call| !matches!(call.receiver, Receiver::Unknown));
        if self.has_strangers(declaration) && !resolved {
            return Vec::new();
        }
        calls
    }

    // Si algo del proyecto puede estar usando el método: una llamada con su nombre (de cualquier
    // aridad) que pueda ir a su clase, una referencia o un texto que lo nombra.
    pub fn is_named(&self, declaration: &Declaration) -> bool {
        let name = declaration.name.as_str();
        self.named.contains(name)
            || self.calls_by_name.get(name).is_some_and(|calls| {
                calls
                    .iter()
                    .any(|call| self.reaches(call, &declaration.owner.name))
            })
    }

    // IntelliJ marca un valor sólo si se puede llevar adentro del método: un literal, una constante
    // de una clase del mismo archivo, una pública (o de afuera del proyecto), o una de paquete del
    // mismo paquete que no esté en los tests si el método no lo está.
    pub fn is_movable_into(&self, value: &str, file: &IndexedSummary<'_>) -> bool {
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
}

fn ancestors_of<'summary>(
    name: &'summary str,
    parents: &HashMap<&str, Vec<&'summary str>>,
) -> HashSet<&'summary str> {
    let mut found = HashSet::new();
    let mut pending: Vec<&str> = parents.get(name).cloned().unwrap_or_default();
    while let Some(parent) = pending.pop() {
        if found.insert(parent) {
            pending.extend(parents.get(parent).cloned().unwrap_or_default());
        }
    }
    found
}

// Las variables de tipo van en mayúsculas cortas (`T`, `E`, `K2`): su clase real no se sabe.
fn is_type_variable(name: &str) -> bool {
    name.chars()
        .all(|letter| letter.is_ascii_uppercase() || letter.is_ascii_digit())
}
