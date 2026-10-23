use tree_sitter::Node;

use super::java::Position;
use crate::lint::java_calls::{Visibility, visibility};
use crate::syntax::{descendants, named_children, one_based};

const TYPE_DECLARATIONS: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "record_declaration",
    "annotation_type_declaration",
];
const RECORD: &str = "record_declaration";
const METHOD: &str = "method_declaration";
const CONSTRUCTOR: &str = "constructor_declaration";
const FIELD: &str = "field_declaration";
const ENUM_BODY_DECLARATIONS: &str = "enum_body_declarations";
const FORMAL_PARAMETERS: &str = "formal_parameters";
const FORMAL_PARAMETER: &str = "formal_parameter";
const SPREAD_PARAMETER: &str = "spread_parameter";
const TYPE_IDENTIFIER: &str = "type_identifier";
const IDENTIFIER: &str = "identifier";
const TYPE_SUFFIX: &str = "type";
const SCOPED_TYPE: &str = "scoped_type_identifier";
const TYPE_PARAMETERS: &str = "type_parameters";
const TYPE_PARAMETER: &str = "type_parameter";
const NAME_FIELD: &str = "name";
const BODY_FIELD: &str = "body";
const TYPE_FIELD: &str = "type";
const PARAMETERS_FIELD: &str = "parameters";
const TYPE_PARAMETERS_FIELD: &str = "type_parameters";

// Un tipo del archivo con la visibilidad que tiene de verdad: la menor entre la suya y la de los
// tipos que lo contienen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredType {
    pub name: String,
    pub top_level: bool,
    pub reach: Visibility,
}

// Un tipo escrito en la firma de un miembro (lo que devuelve, sus parámetros, el tipo de un campo),
// con la visibilidad efectiva del miembro y los parámetros de tipo que lo tapan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exposure {
    pub written: String,
    pub position: Position,
    pub reach: Visibility,
    pub type_parameters: Vec<String>,
}

pub const fn rank(visibility: Visibility) -> u8 {
    match visibility {
        Visibility::Private => 0,
        Visibility::Package => 1,
        Visibility::Protected => 2,
        Visibility::Public => 3,
    }
}

const fn narrower(first: Visibility, second: Visibility) -> Visibility {
    if rank(first) <= rank(second) {
        first
    } else {
        second
    }
}

// Lo que un tipo hereda del que lo contiene: hasta dónde se ve y los parámetros de tipo a la vista.
struct Enclosing {
    reach: Visibility,
    parameters: Vec<String>,
}

struct Walker<'source> {
    source: &'source [u8],
    types: Vec<DeclaredType>,
    exposures: Vec<Exposure>,
}

// Los tipos del archivo y lo que exponen sus miembros no privados, para `ClassEscapesItsScope`.
pub fn exposures(root: Node<'_>, source: &[u8]) -> (Vec<DeclaredType>, Vec<Exposure>) {
    let mut walker = Walker {
        source,
        types: Vec::new(),
        exposures: Vec::new(),
    };
    for declaration in named_children(root)
        .into_iter()
        .filter(|node| TYPE_DECLARATIONS.contains(&node.kind()))
    {
        let enclosing = Enclosing {
            reach: Visibility::Public,
            parameters: Vec::new(),
        };
        walker.declared_type(declaration, &enclosing, true);
    }
    (walker.types, walker.exposures)
}

impl Walker<'_> {
    fn text(&self, node: Node<'_>) -> String {
        node.utf8_text(self.source).unwrap_or_default().to_owned()
    }

    fn declared_type(&mut self, declaration: Node<'_>, enclosing: &Enclosing, top_level: bool) {
        let reach = narrower(visibility(declaration, self.source), enclosing.reach);
        if let Some(name) = declaration.child_by_field_name(NAME_FIELD) {
            self.types.push(DeclaredType {
                name: self.text(name),
                top_level,
                reach,
            });
        }
        let mut parameters = enclosing.parameters.clone();
        parameters.extend(self.type_parameters(declaration));
        if declaration.kind() == RECORD
            && let Some(components) = declaration.child_by_field_name(PARAMETERS_FIELD)
        {
            self.expose_parameters(components, reach, &parameters);
        }
        let Some(body) = declaration.child_by_field_name(BODY_FIELD) else {
            return;
        };
        let members = named_children(body).into_iter().flat_map(|member| {
            if member.kind() == ENUM_BODY_DECLARATIONS {
                named_children(member)
            } else {
                vec![member]
            }
        });
        let inner = Enclosing { reach, parameters };
        for member in members {
            self.member(member, &inner);
        }
    }

    fn member(&mut self, member: Node<'_>, enclosing: &Enclosing) {
        if TYPE_DECLARATIONS.contains(&member.kind()) {
            self.declared_type(member, enclosing, false);
            return;
        }
        let (owner, parameters) = (enclosing.reach, enclosing.parameters.as_slice());
        if !matches!(member.kind(), METHOD | CONSTRUCTOR | FIELD) {
            return;
        }
        let reach = narrower(visibility(member, self.source), owner);
        if reach == Visibility::Private {
            return;
        }
        let mut in_scope = parameters.to_vec();
        in_scope.extend(self.type_parameters(member));
        if let Some(typ) = member.child_by_field_name(TYPE_FIELD) {
            self.expose(typ, reach, &in_scope);
        }
        if let Some(list) = member.child_by_field_name(PARAMETERS_FIELD) {
            self.expose_parameters(list, reach, &in_scope);
        }
    }

    fn expose_parameters(&mut self, list: Node<'_>, reach: Visibility, parameters: &[String]) {
        if list.kind() != FORMAL_PARAMETERS {
            return;
        }
        for parameter in named_children(list)
            .into_iter()
            .filter(|node| matches!(node.kind(), FORMAL_PARAMETER | SPREAD_PARAMETER))
        {
            let typ = parameter.child_by_field_name(TYPE_FIELD).or_else(|| {
                named_children(parameter)
                    .into_iter()
                    .find(|child| child.kind().ends_with(TYPE_SUFFIX) || child.kind() == TYPE_IDENTIFIER)
            });
            if let Some(typ) = typ {
                self.expose(typ, reach, parameters);
            }
        }
    }

    // Cada nombre de tipo dentro de un tipo escrito: `List<Entrada>` expone `Entrada`.
    fn expose(&mut self, typ: Node<'_>, reach: Visibility, parameters: &[String]) {
        let names: Vec<Node<'_>> = if typ.kind() == TYPE_IDENTIFIER || typ.kind() == SCOPED_TYPE {
            vec![typ]
        } else {
            descendants(typ)
                .into_iter()
                .filter(|node| node.kind() == TYPE_IDENTIFIER || node.kind() == SCOPED_TYPE)
                .filter(|node| node.parent().is_none_or(|parent| parent.kind() != SCOPED_TYPE))
                .collect()
        };
        for name in names {
            let start = name.start_position();
            self.exposures.push(Exposure {
                written: self.text(name),
                position: Position {
                    line: one_based(start.row),
                    column: one_based(start.column),
                },
                reach,
                type_parameters: parameters.to_vec(),
            });
        }
    }

    fn type_parameters(&self, declaration: Node<'_>) -> Vec<String> {
        declaration
            .child_by_field_name(TYPE_PARAMETERS_FIELD)
            .or_else(|| {
                named_children(declaration)
                    .into_iter()
                    .find(|child| child.kind() == TYPE_PARAMETERS)
            })
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|parameter| parameter.kind() == TYPE_PARAMETER)
            .filter_map(|parameter| {
                named_children(parameter)
                    .into_iter()
                    .find(|child| child.kind() == TYPE_IDENTIFIER || child.kind() == IDENTIFIER)
            })
            .map(|name| self.text(name))
            .collect()
    }
}
