use std::collections::HashSet;

use super::java::{Declaration, JavaSummary};
use super::java_nullness::{ProjectTypes, Resolved, parents_of, resolve};
use crate::jvm::class_file::ClassFile;
use crate::jvm::library::Library;
use crate::jvm::method::Method;
use crate::lint::java_calls::{Arity, ReturnType};
use crate::lint::java_throws::{CallRoot, CallStep, ThrowOrigin};

const JAVA_OBJECT: &str = "java.lang.Object";
const CONSTRUCTOR_NAME: &str = "<init>";
const CLOSE: &str = "close";
const WILDCARD: &str = "*";
const QUALIFIER_SEPARATOR: char = '.';
const DESCRIPTOR_CLOSE: char = ')';
const MAX_RECEIVERS: usize = 64;

// Lo que una firma candidata dice de una llamada.
#[derive(Debug, Clone)]
struct Signature {
    thrown: Vec<Resolved>,
    returned: Option<Resolved>,
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
    types: &'query ProjectTypes<'summary>,
    library: &'query Library,
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
            ThrowOrigin::Thrown(written) => Some(vec![self.named(summary, written)]),
            ThrowOrigin::Creation(written, arity) => self
                .constructors(&self.named(summary, written), *arity)
                .map(|signatures| all_thrown(&signatures)),
            ThrowOrigin::Close(written) => match self.members(&self.named(summary, written), CLOSE, 0) {
                Search::Found(signatures) => Some(all_thrown(&signatures)),
                Search::Absent | Search::Incomplete => None,
            },
            ThrowOrigin::Call(root, steps) => self.call_thrown(summary, root, steps),
        }
    }

    fn call_thrown(
        &self,
        summary: &JavaSummary,
        root: &CallRoot,
        steps: &[CallStep],
    ) -> Option<Vec<Resolved>> {
        let (first, rest) = steps.split_first()?;
        let mut signatures = match root {
            CallRoot::Scope(enclosing) => self.in_scope(summary, enclosing, first)?,
            CallRoot::Type(written) => self.on_receivers(&[self.named(summary, written)], first)?,
        };
        let mut thrown = all_thrown(&signatures);
        for step in rest {
            let receivers = returned_by(&signatures)?;
            signatures = self.on_receivers(&receivers, step)?;
            thrown.extend(all_thrown(&signatures));
        }
        Some(thrown)
    }

    fn on_receivers(&self, receivers: &[Resolved], step: &CallStep) -> Option<Vec<Signature>> {
        let mut found = Vec::new();
        for receiver in receivers {
            match self.members(receiver, &step.name, step.arity) {
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
        step: &CallStep,
    ) -> Option<Vec<Signature>> {
        for owner in enclosing {
            match self.members(&self.named(summary, owner), &step.name, step.arity) {
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
            .filter(|(_, member)| *member == step.name || *member == WILDCARD)
            .map(|(class, _)| class)
        {
            match self.members(&self.named(summary, class), &step.name, step.arity) {
                Search::Found(signatures) => found.extend(signatures),
                Search::Incomplete => return None,
                Search::Absent => {}
            }
        }
        (!found.is_empty()).then_some(found)
    }

    fn members(&self, start: &Resolved, name: &str, arity: usize) -> Search {
        let mut pending = vec![start.clone()];
        let mut seen = HashSet::new();
        let mut collected = Collected::default();
        while let Some(current) = pending.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            let Some(parents) = self.visit(&current, (name, arity), &mut collected) else {
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
    fn visit(
        &self,
        current: &Resolved,
        wanted: (&str, usize),
        collected: &mut Collected,
    ) -> Option<Vec<Resolved>> {
        let (name, arity) = wanted;
        match current {
            Resolved::Project(owner) => {
                collected.found.extend(self.project_members(owner, name, arity));
                let mut parents = vec![Resolved::Library(JAVA_OBJECT.to_owned())];
                parents.extend(parents_of(self.types, self.library, owner));
                Some(parents)
            }
            Resolved::Library(class) => {
                let file = self.library.class_or_jdk(class)?;
                collected
                    .found
                    .extend(library_members(&file, name, arity, &mut collected.overridden));
                let mut parents: Vec<Resolved> =
                    file.interfaces.iter().cloned().map(Resolved::Library).collect();
                parents.extend(file.superclass.iter().cloned().map(Resolved::Library));
                Some(parents)
            }
        }
    }

    fn project_members(&self, owner: &str, name: &str, arity: usize) -> Vec<Signature> {
        self.types
            .declarations_of(owner)
            .filter(|(_, declaration)| {
                !declaration.is_constructor && declaration.name == name && accepts(declaration, arity)
            })
            .map(|(summary, declaration)| self.project_signature(summary, declaration))
            .collect()
    }

    fn project_signature(&self, summary: &JavaSummary, declaration: &Declaration) -> Signature {
        Signature {
            thrown: declared_clauses(summary, declaration)
                .map(|written| self.named(summary, written))
                .collect(),
            returned: (declaration.return_type == ReturnType::Reference)
                .then(|| self.named(summary, &declaration.returned)),
        }
    }

    // Una clase sin constructores escritos tiene el implícito; una interfaz, en `new X() { }`, nada.
    fn constructors(&self, class: &Resolved, arity: usize) -> Option<Vec<Signature>> {
        let found = match class {
            Resolved::Project(owner) => self
                .types
                .declarations_of(owner)
                .filter(|(_, declaration)| declaration.is_constructor && accepts(declaration, arity))
                .map(|(summary, declaration)| self.project_signature(summary, declaration))
                .collect(),
            Resolved::Library(name) => {
                let file = self.library.class_or_jdk(name)?;
                library_members(&file, CONSTRUCTOR_NAME, arity, &mut HashSet::new())
            }
        };
        (!found.is_empty() || arity == 0).then_some(found)
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
                Resolved::Project(owner) => pending.extend(parents_of(self.types, self.library, owner)),
                Resolved::Library(name) => match self.library.class_or_jdk(name) {
                    Some(file) => pending.extend(file.superclass.iter().cloned().map(Resolved::Library)),
                    None => complete = false,
                },
            }
        }
        complete.then_some(false)
    }
}

fn all_thrown(signatures: &[Signature]) -> Vec<Resolved> {
    signatures
        .iter()
        .flat_map(|signature| signature.thrown.iter().cloned())
        .collect()
}

// Las clases sobre las que sigue la cadena. Un paso que devuelve un primitivo o `void` no sigue, y
// demasiadas sobrecargas distintas (`assertThat`) no se persiguen.
fn returned_by(signatures: &[Signature]) -> Option<Vec<Resolved>> {
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
    name: &str,
    arity: usize,
    overridden: &mut HashSet<String>,
) -> Vec<Signature> {
    file.methods
        .iter()
        .filter(|method| method.name == name && method.accepts(arity) && !method.is_private())
        .filter(|method| overridden.insert(parameters_of(method).to_owned()))
        .map(|method| Signature {
            thrown: method.exceptions.iter().cloned().map(Resolved::Library).collect(),
            returned: method.return_class().map(Resolved::Library),
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

const fn accepts(declaration: &Declaration, arity: usize) -> bool {
    let parameters = declaration.parameters.len();
    match declaration.arity {
        Arity::Fixed => arity == parameters,
        Arity::Variadic => arity.saturating_add(1) >= parameters,
    }
}

// Los tipos que la declaración escribió en su `throws`.
pub fn declared_clauses<'summary>(
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
