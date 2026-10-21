use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use super::class_file::{ClassFile, Method, parse};
use super::zip::Archive;

const CLASS_EXTENSION: &str = ".class";
const PACKAGE_INFO: &str = "package-info";
const QUALIFIER_SEPARATOR: char = '.';
const PATH_SEPARATOR: char = '/';
const NESTED_SEPARATOR: char = '$';
const NULL_MARKED: &str = "NullMarked";
const NON_NULL_API: &str = "NonNullApi";
const NULL_MARKED_LABEL: &str = "@NullMarked";
const NON_NULL_API_LABEL: &str = "@NonNullApi";

// Las clases de las dependencias, leídas a pedido: el índice de cada jar al cargar, y cada clase
// la primera vez que se la busca.
pub struct Library {
    archives: Vec<Archive>,
    owners: HashMap<String, usize>,
    classes: RefCell<HashMap<String, Option<Rc<ClassFile>>>>,
}

impl Library {
    pub fn load(jars: &[PathBuf]) -> Self {
        let archives: Vec<Archive> = jars.iter().filter_map(|jar| Archive::open(jar).ok()).collect();
        let mut owners = HashMap::new();
        for (index, archive) in archives.iter().enumerate() {
            for name in archive.names().filter(|name| name.ends_with(CLASS_EXTENSION)) {
                owners.entry(name.to_owned()).or_insert(index);
            }
        }
        Self {
            archives,
            owners,
            classes: RefCell::new(HashMap::new()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }

    // Una clase por su nombre de Java: `a.b.C` y también la anidada `a.b.C.D` (`a/b/C$D.class`).
    pub fn class(&self, name: &str) -> Option<Rc<ClassFile>> {
        if let Some(known) = self.classes.borrow().get(name) {
            return known.clone();
        }
        let found = entry_candidates(name)
            .into_iter()
            .find_map(|entry| self.read_class(&entry));
        self.classes.borrow_mut().insert(name.to_owned(), found.clone());
        found
    }

    pub fn has_class(&self, name: &str) -> bool {
        entry_candidates(name)
            .iter()
            .any(|entry| self.owners.contains_key(entry))
    }

    fn read_class(&self, entry: &str) -> Option<Rc<ClassFile>> {
        let archive = self.archives.get(*self.owners.get(entry)?)?;
        parse(&archive.read(entry)?).ok().map(Rc::new)
    }

    // Si los parámetros y retornos de la clase son no nulos salvo que digan lo contrario, y por
    // qué: `@NullMarked` de JSpecify (Spring 7) o `@NonNullApi` de Spring (6), en la clase, en
    // las que la encierran o en el paquete.
    pub fn null_default(&self, class: &ClassFile) -> Option<&'static str> {
        let mut scopes = vec![class.name.clone()];
        let mut outer = class.name.as_str();
        while let Some((enclosing, _)) = outer.rsplit_once(QUALIFIER_SEPARATOR) {
            scopes.push(format!("{enclosing}{QUALIFIER_SEPARATOR}{PACKAGE_INFO}"));
            scopes.push(enclosing.to_owned());
            outer = enclosing;
        }
        scopes
            .iter()
            .filter_map(|scope| self.class(scope))
            .find_map(|found| label_of(&found.annotations))
    }

    // El método que redefine uno de esta clase: el primero con ese nombre y cantidad de
    // parámetros, subiendo por la clase padre y las interfaces.
    pub fn inherited_method(&self, class: &str, name: &str, arity: usize) -> Option<(Rc<ClassFile>, Method)> {
        let mut pending = vec![class.to_owned()];
        let mut seen = HashSet::new();
        while let Some(current) = pending.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            let Some(found) = self.class(&current) else {
                continue;
            };
            if let Some(method) = found.methods.iter().find(|method| {
                method.name == name
                    && method.parameter_count() == arity
                    && !method.is_private
                    && !method.is_static
            }) {
                return Some((Rc::clone(&found), method.clone()));
            }
            pending.extend(found.interfaces.iter().cloned());
            pending.extend(found.superclass.clone());
        }
        None
    }
}

fn label_of(annotations: &[String]) -> Option<&'static str> {
    annotations
        .iter()
        .find_map(|annotation| match annotation.rsplit(QUALIFIER_SEPARATOR).next() {
            Some(NULL_MARKED) => Some(NULL_MARKED_LABEL),
            Some(NON_NULL_API) => Some(NON_NULL_API_LABEL),
            _ => None,
        })
}

// Las entradas posibles para un nombre de Java: con cada punto final pasado a `$`, de la clase de
// nivel superior a la más anidada.
fn entry_candidates(name: &str) -> Vec<String> {
    let path = name.replace(QUALIFIER_SEPARATOR, &PATH_SEPARATOR.to_string());
    let mut candidates = vec![format!("{path}{CLASS_EXTENSION}")];
    let mut nested = path;
    while let Some(last) = nested.rfind(PATH_SEPARATOR) {
        nested.replace_range(last..=last, &NESTED_SEPARATOR.to_string());
        candidates.push(format!("{nested}{CLASS_EXTENSION}"));
    }
    candidates
}
