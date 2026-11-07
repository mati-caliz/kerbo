use super::java::JavaSummary;
use super::java_nullness::Resolved;
use super::java_throw_lookup::ThrowLookup;
use super::java_typed::{Typed, library_field};

const QUALIFIER_SEPARATOR: char = '.';
const WILDCARD: &str = "*";

// De dónde sale el tipo de la raíz de una cadena y quiénes son los padres de un tipo del proyecto.
impl ThrowLookup<'_, '_> {
    // Un nombre en mayúsculas puede ser una constante de un `import static`, y `Clase.CAMPO` un
    // campo: se siguen por su tipo.
    pub(super) fn root_type(&self, summary: &JavaSummary, written: &str) -> Typed {
        self.imported_constant(summary, written)
            .or_else(|| self.qualified_field(summary, written))
            .unwrap_or_else(|| Typed::raw(self.named(summary, written)))
    }

    fn qualified_field(&self, summary: &JavaSummary, written: &str) -> Option<Typed> {
        let (class, field) = written.rsplit_once(QUALIFIER_SEPARATOR)?;
        self.field_type(&self.named(summary, class), field)
    }

    fn field_type(&self, class: &Resolved, field: &str) -> Option<Typed> {
        match class {
            Resolved::Project(owner) => self
                .types
                .field_of(owner, field)
                .map(|(declaring, found)| Typed::raw(self.named(declaring, &found.type_name))),
            Resolved::Library(name) => library_field(&*self.library.class_or_jdk(name)?, field),
        }
    }

    fn imported_constant(&self, summary: &JavaSummary, name: &str) -> Option<Typed> {
        if name.contains(QUALIFIER_SEPARATOR) {
            return None;
        }
        summary
            .static_imports
            .iter()
            .filter_map(|path| path.rsplit_once(QUALIFIER_SEPARATOR))
            .filter(|(_, member)| *member == name || *member == WILDCARD)
            .find_map(|(class, _)| self.field_type(&self.named(summary, class), name))
    }

    pub(super) fn project_parents(&self, owner: &str) -> Vec<Resolved> {
        self.types
            .written_parents(owner)
            .map(|(summary, parent)| self.parent_named(summary, parent))
            .collect()
    }

    // Un padre se escribe sin calificar aunque sea una clase anidada de lo importado: `Map.Entry`
    // queda como `Entry` con `import java.util.Map`.
    fn parent_named(&self, summary: &JavaSummary, written: &str) -> Resolved {
        let found = self.named(summary, written);
        let Resolved::Library(name) = &found else {
            return found;
        };
        if self.library.has_class(name) {
            return found;
        }
        summary
            .imports
            .iter()
            .map(|imported| format!("{imported}{QUALIFIER_SEPARATOR}{written}"))
            .find(|nested| self.library.has_class(nested))
            .map_or(found, Resolved::Library)
    }
}
