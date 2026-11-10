use std::collections::HashSet;

use super::java::{Declaration, JavaSummary};
use super::java_nullness::{ProjectTypes, Resolved, resolve};
use super::java_throw_functional::step_thrown;
use super::java_throw_project::{raw_signature, visit_project};
use super::java_throw_wanted::Wanted;
use super::java_typed::{Typed, library_parents, library_returned};
use crate::jvm::class_file::ClassFile;
use crate::jvm::library::Library;
use crate::jvm::method::Method;
use crate::lint::java_calls::Arity;
use crate::lint::java_throws::{CallRoot, CallStep, ThrowOrigin};

const CONSTRUCTOR_NAME: &str = "<init>";
const CLOSE: &str = "close";
const WILDCARD: &str = "*";
const QUALIFIER_SEPARATOR: char = '.';
const DESCRIPTOR_CLOSE: char = ')';
const MAX_RECEIVERS: usize = 64;

// Lo que una firma candidata dice de una llamada. Cada elemento de `carried` es un `throws E` que
// depende de lo que tiren las lambdas o referencias a método pasadas en esas posiciones.
#[derive(Debug, Clone)]
pub(super) struct Signature {
    pub(super) thrown: Vec<Resolved>,
    pub(super) returned: Option<Typed>,
    pub(super) carried: Vec<Vec<usize>>,
    pub(super) unresolved: bool,
}

#[derive(Default)]
struct Collected {
    found: Vec<Signature>,
    overridden: HashSet<String>,
}

// `Incomplete` es que algún tipo de la jerarquía no se pudo leer: puede haber sobrecargas sin ver.
enum Search {
    Found(Vec<Signature>),
    Absent,
    Incomplete,
}

// Qué tira cada cosa, sin compilar: el método se busca por nombre y cantidad de argumentos y se
// juntan todas las sobrecargas candidatas. Así puede faltar un aviso, nunca sobrar: la sobrecarga
// que tira hace callar a la regla aunque el compilador hubiera elegido otra.
pub struct ThrowLookup<'query, 'summary> {
    pub(super) types: &'query ProjectTypes<'summary>,
    pub(super) library: &'query Library,
}

impl<'query, 'summary> ThrowLookup<'query, 'summary> {
    pub const fn new(types: &'query ProjectTypes<'summary>, library: &'query Library) -> Self {
        Self { types, library }
    }

    // El nombre escrito en un archivo. `Afuera.Adentro` se resuelve por `Afuera`.
    pub fn named(&self, summary: &JavaSummary, written: &str) -> Resolved {
        let Some((head, rest)) = written.split_once(QUALIFIER_SEPARATOR) else {
            return resolve(self.types, self.library, summary, written);
        };
        let last = written.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(written);
        if head.starts_with(|first: char| first.is_ascii_uppercase()) {
            return match resolve(self.types, self.library, summary, head) {
                Resolved::Library(outer) => Resolved::Library(format!("{outer}{QUALIFIER_SEPARATOR}{rest}")),
                Resolved::Project(_) => Resolved::Project(last.to_owned()),
            };
        }
        if !self.library.has_class(written) && self.types.is_declared(last) {
            return Resolved::Project(last.to_owned());
        }
        Resolved::Library(written.to_owned())
    }

    // Lo que puede tirar un punto de un cuerpo, o nada si no se puede saber.
    pub fn thrown_by(&self, summary: &JavaSummary, origin: &ThrowOrigin) -> Option<Vec<Resolved>> {
        match origin {
            ThrowOrigin::Unknown => None,
            ThrowOrigin::Thrown(written) | ThrowOrigin::Rethrown(written) => {
                Some(vec![self.named(summary, written)])
            }
            ThrowOrigin::Creation(written, arity) => self
                .constructors(&self.named(summary, written), Some(*arity))
                .and_then(|signatures| all_thrown(&signatures)),
            ThrowOrigin::Close(written) => {
                match self.members(
                    &Typed::raw(self.named(summary, written)),
                    Wanted::by_arity(CLOSE, 0),
                ) {
                    Search::Found(signatures) => all_thrown(&signatures),
                    Search::Absent | Search::Incomplete => None,
                }
            }
            ThrowOrigin::Call(root, steps) => self.call_thrown(summary, root, steps),
        }
    }

    // Las clases donde se busca el último método de una llamada, o nada si no se sabe.
    pub fn receivers_of(&self, summary: &JavaSummary, origin: &ThrowOrigin) -> Option<Vec<Resolved>> {
        let ThrowOrigin::Call(root, steps) = origin else {
            return None;
        };
        let (last, before) = steps.split_last()?;
        let Some((first, rest)) = before.split_first() else {
            return match root {
                CallRoot::Scope(enclosing) => self.scope_owner(summary, enclosing, last),
                CallRoot::Type(_) | CallRoot::Generic(..) => Some(vec![self.root_type(summary, root)?.class]),
            };
        };
        let mut signatures = self.on_root(summary, root, Wanted::of_step(first))?;
        for step in rest {
            signatures = self.on_receivers(&returned_by(&signatures)?, Wanted::of_step(step))?;
        }
        Some(
            returned_by(&signatures)?
                .into_iter()
                .map(|receiver| receiver.class)
                .collect(),
        )
    }

    fn scope_owner(
        &self,
        summary: &JavaSummary,
        enclosing: &[String],
        step: &CallStep,
    ) -> Option<Vec<Resolved>> {
        for owner in enclosing {
            let class = self.named(summary, owner);
            match self.members(&Typed::raw(class.clone()), Wanted::of_step(step)) {
                Search::Found(_) => return Some(vec![class]),
                Search::Incomplete => return None,
                Search::Absent => {}
            }
        }
        None
    }

    fn call_thrown(
        &self,
        summary: &JavaSummary,
        root: &CallRoot,
        steps: &[CallStep],
    ) -> Option<Vec<Resolved>> {
        let (first, rest) = steps.split_first()?;
        let mut signatures = self.on_root(summary, root, Wanted::of_step(first))?;
        let mut thrown = step_thrown(self, summary, &signatures, first)?;
        for step in rest {
            let receivers = returned_by(&signatures)?;
            signatures = self.on_receivers(&receivers, Wanted::of_step(step))?;
            thrown.extend(step_thrown(self, summary, &signatures, step)?);
        }
        Some(thrown)
    }

    pub(super) fn on_root(
        &self,
        summary: &JavaSummary,
        root: &CallRoot,
        wanted: Wanted<'_>,
    ) -> Option<Vec<Signature>> {
        match root {
            CallRoot::Scope(enclosing) => self.in_scope(summary, enclosing, wanted),
            CallRoot::Type(_) | CallRoot::Generic(..) => {
                self.on_receivers(&[self.root_type(summary, root)?], wanted)
            }
        }
    }

    fn on_receivers(&self, receivers: &[Typed], wanted: Wanted<'_>) -> Option<Vec<Signature>> {
        let mut found = Vec::new();
        for receiver in receivers {
            match self.members(receiver, wanted) {
                Search::Found(signatures) => found.extend(signatures),
                Search::Absent | Search::Incomplete => return None,
            }
        }
        Some(found)
    }

    // Una llamada sin receptor va a la clase más cercana que tenga un método con ese nombre, y si
    // ninguna lo tiene, a un `import static`.
    fn in_scope(
        &self,
        summary: &JavaSummary,
        enclosing: &[String],
        wanted: Wanted<'_>,
    ) -> Option<Vec<Signature>> {
        for owner in enclosing {
            match self.members(&Typed::raw(self.named(summary, owner)), wanted) {
                Search::Found(signatures) => return Some(signatures),
                Search::Incomplete => return None,
                Search::Absent => {}
            }
        }
        let mut found = Vec::new();
        for class in summary
            .static_imports
            .iter()
            .filter_map(|path| path.rsplit_once(QUALIFIER_SEPARATOR))
            .filter(|(_, member)| *member == wanted.name || *member == WILDCARD)
            .map(|(class, _)| class)
        {
            match self.members(&Typed::raw(self.named(summary, class)), wanted) {
                Search::Found(signatures) => found.extend(signatures),
                Search::Incomplete => return None,
                Search::Absent => {}
            }
        }
        (!found.is_empty()).then_some(found)
    }

    fn members(&self, start: &Typed, wanted: Wanted<'_>) -> Search {
        let mut pending = vec![start.clone()];
        let mut seen = HashSet::new();
        let mut collected = Collected::default();
        while let Some(current) = pending.pop() {
            if !seen.insert(current.class.clone()) {
                continue;
            }
            let Some(parents) = self.visit(&current, wanted, &mut collected) else {
                return Search::Incomplete;
            };
            pending.extend(parents);
        }
        if collected.found.is_empty() {
            Search::Absent
        } else {
            Search::Found(collected.found)
        }
    }

    // Junta lo que declara un tipo y devuelve de quién hereda, o nada si no se puede leer.
    fn visit(&self, current: &Typed, wanted: Wanted<'_>, collected: &mut Collected) -> Option<Vec<Typed>> {
        match &current.class {
            Resolved::Project(owner) => {
                let (found, parents) = visit_project(self, owner, current, wanted);
                collected.found.extend(found);
                Some(parents)
            }
            Resolved::Library(class) => {
                let file = self.library.class_or_jdk(class)?;
                collected
                    .found
                    .extend(library_members(&file, current, wanted, &mut collected.overridden));
                Some(library_parents(&file, current))
            }
        }
    }

    // Una clase sin constructores escritos tiene el implícito; una interfaz, en `new X() { }`, nada.
    // Sin `arity` es `Tipo::new`, que puede ir a cualquiera.
    pub(super) fn constructors(&self, class: &Resolved, arity: Option<usize>) -> Option<Vec<Signature>> {
        let wanted = Wanted {
            name: CONSTRUCTOR_NAME,
            arity,
            kinds: &[],
        };
        let found: Vec<Signature> = match class {
            Resolved::Project(owner) => self
                .types
                .declarations_of(owner)
                .filter(|(_, declaration)| declaration.is_constructor && wanted.admits(declaration))
                .map(|(summary, declaration)| raw_signature(self, summary, declaration))
                .collect(),
            Resolved::Library(name) => {
                let file = self.library.class_or_jdk(name)?;
                library_members(&file, &Typed::raw(class.clone()), wanted, &mut HashSet::new())
            }
        };
        let implicit = arity.map_or(matches!(class, Resolved::Project(_)), |count| count == 0);
        (!found.is_empty() || implicit).then_some(found)
    }

    // Si `class` es `ancestor` o hereda de él. Sin leer toda la jerarquía, sólo un sí es seguro.
    pub fn is_subclass(&self, class: &Resolved, ancestor: &Resolved) -> Option<bool> {
        let mut pending = vec![class.clone()];
        let mut seen = HashSet::new();
        let mut complete = true;
        while let Some(current) = pending.pop() {
            if current == *ancestor {
                return Some(true);
            }
            if !seen.insert(current.clone()) {
                continue;
            }
            match &current {
                Resolved::Project(owner) => pending.extend(self.project_parents(owner)),
                Resolved::Library(name) => match self.library.class_or_jdk(name) {
                    Some(file) => pending.extend(file.superclass.iter().cloned().map(Resolved::Library)),
                    None => complete = false,
                },
            }
        }
        complete.then_some(false)
    }
}

// Sin los argumentos a mano, un `throws E` que depende de ellos no se sabe qué es.
pub(super) fn all_thrown(signatures: &[Signature]) -> Option<Vec<Resolved>> {
    signatures
        .iter()
        .all(|signature| signature.carried.is_empty() && !signature.unresolved)
        .then(|| {
            signatures
                .iter()
                .flat_map(|signature| signature.thrown.iter().cloned())
                .collect()
        })
}

// Las clases sobre las que sigue la cadena. Un paso que devuelve un primitivo o `void` no sigue, y
// demasiadas sobrecargas distintas (`assertThat`) no se persiguen.
fn returned_by(signatures: &[Signature]) -> Option<Vec<Typed>> {
    let mut receivers = Vec::new();
    for signature in signatures {
        let returned = signature.returned.as_ref()?;
        if !receivers.contains(returned) {
            receivers.push(returned.clone());
        }
    }
    (receivers.len() <= MAX_RECEIVERS).then_some(receivers)
}

// Un método redefinido con los mismos parámetros tapa al de arriba: `ByteArrayOutputStream.close()`
// no tira lo que tira `OutputStream.close()`.
fn library_members(
    file: &ClassFile,
    current: &Typed,
    wanted: Wanted<'_>,
    overridden: &mut HashSet<String>,
) -> Vec<Signature> {
    file.methods
        .iter()
        .filter(|method| {
            method.name == wanted.name
                && wanted.admits_method(method)
                && !method.is_private()
                && !wanted.rules_out(method)
        })
        .filter(|method| overridden.insert(parameters_of(method).to_owned()))
        .map(|method| Signature {
            thrown: method.exceptions.iter().cloned().map(Resolved::Library).collect(),
            returned: library_returned(file, method, current),
            carried: Vec::new(),
            unresolved: false,
        })
        .collect()
}

fn parameters_of(method: &Method) -> &str {
    method
        .descriptor
        .split(DESCRIPTOR_CLOSE)
        .next()
        .unwrap_or(&method.descriptor)
}

pub(super) const fn accepts(declaration: &Declaration, arity: usize) -> bool {
    let parameters = declaration.parameters.len();
    match declaration.arity {
        Arity::Fixed => arity == parameters,
        Arity::Variadic => arity.saturating_add(1) >= parameters,
    }
}

// Los tipos que la declaración escribió en su `throws`.
pub(super) fn declared_clauses<'summary>(
    summary: &'summary JavaSummary,
    declaration: &'summary Declaration,
) -> impl Iterator<Item = &'summary str> {
    summary
        .method_throws
        .iter()
        .filter(|method| {
            method.line == declaration.position.line && method.column == declaration.position.column
        })
        .flat_map(|method| method.clauses.iter())
        .map(|clause| clause.written.as_str())
}
