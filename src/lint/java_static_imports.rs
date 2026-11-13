use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_receivers::{Receiver, Variables, receiver_of};
use crate::syntax::named_children;

const IMPORT: &str = "import_declaration";
const ASTERISK: &str = "asterisk";
const STATIC: &str = "static ";
const OBJECT_FIELD: &str = "object";
const QUALIFIER_SEPARATOR: char = '.';

// Un método importado con `import static` (o con un comodín) se llama sin calificar desde afuera de
// su clase: esas llamadas no se sabe adónde van, salvo que un import lo nombre con su clase.
pub struct StaticImports {
    members: Vec<ImportedMember>,
    any: bool,
}

struct ImportedMember {
    class: String,
    name: String,
}

impl StaticImports {
    pub fn of(top_level: Node<'_>, source: &[u8]) -> Self {
        let mut imports = Self {
            members: Vec::new(),
            any: false,
        };
        let declarations = top_level.parent().map(named_children).unwrap_or_default();
        for import in declarations
            .into_iter()
            .filter(|node| node.kind() == IMPORT && text(*node, source).contains(STATIC))
        {
            let children = named_children(import);
            if children.iter().any(|child| child.kind() == ASTERISK) {
                imports.any = true;
                continue;
            }
            imports.members.extend(children.iter().filter_map(|path| {
                let (class, name) = text(*path, source).rsplit_once(QUALIFIER_SEPARATOR)?;
                Some(ImportedMember {
                    class: class.to_owned(),
                    name: name.to_owned(),
                })
            }));
        }
        imports
    }

    pub fn receiver(&self, invocation: Node<'_>, source: &[u8], variables: &Variables) -> Receiver {
        let resolved = receiver_of(invocation, source, variables);
        if self.imports(invocation, source) && matches!(resolved, Receiver::Scope(_)) {
            Receiver::Unknown
        } else {
            resolved
        }
    }

    // Como `receiver`, pero la llamada sin calificar a un método que trae un solo import con nombre
    // va a la clase de ese import, escrita con el paquete.
    pub fn receiver_through_class(
        &self,
        invocation: Node<'_>,
        source: &[u8],
        variables: &Variables,
    ) -> Receiver {
        match self.receiver(invocation, source, variables) {
            Receiver::Unknown => self
                .imported_class(invocation, source)
                .map_or(Receiver::Unknown, |class| Receiver::Type(class.to_owned())),
            resolved @ (Receiver::Type(_) | Receiver::Scope(_)) => resolved,
        }
    }

    pub fn imports(&self, invocation: Node<'_>, source: &[u8]) -> bool {
        self.any
            || invocation.child_by_field_name(NAME_FIELD).is_some_and(|name| {
                self.members
                    .iter()
                    .any(|member| member.name == text(name, source))
            })
    }

    pub fn imported_class(&self, invocation: Node<'_>, source: &[u8]) -> Option<&str> {
        if invocation.child_by_field_name(OBJECT_FIELD).is_some() {
            return None;
        }
        let name = text(invocation.child_by_field_name(NAME_FIELD)?, source);
        let mut classes = self
            .members
            .iter()
            .filter(|member| member.name == name)
            .map(|member| member.class.as_str());
        let first = classes.next()?;
        classes.all(|other| other == first).then_some(first)
    }
}
