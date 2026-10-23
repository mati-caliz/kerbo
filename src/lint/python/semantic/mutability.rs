use tree_sitter::Node;

use super::model::{BindingKind, Model, Resolution, ScopeId};
use super::qualified::{is_typing, qualified_name_from};
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ASSIGNMENT, ATTRIBUTE, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT,
    NAME_FIELD, NONE, RIGHT_FIELD, SUBSCRIPT, TUPLE, TYPE_FIELD, VALUE_FIELD, named, text, unparenthesized,
};

// `is_immutable_non_generic_type` de ruff.
const IMMUTABLE_NON_GENERIC: [&str; 11] = [
    "collections.abc.Sized",
    "typing.LiteralString",
    "typing.Sized",
    "builtins.bool",
    "builtins.bytes",
    "builtins.complex",
    "builtins.float",
    "builtins.int",
    "builtins.object",
    "builtins.range",
    "builtins.str",
];
// `is_immutable_generic_type` de ruff.
const IMMUTABLE_GENERIC: [&str; 25] = [
    "builtins.frozenset",
    "builtins.tuple",
    "builtins.frozendict",
    "collections.abc.ByteString",
    "collections.abc.Collection",
    "collections.abc.Container",
    "collections.abc.Iterable",
    "collections.abc.Mapping",
    "collections.abc.Reversible",
    "collections.abc.Sequence",
    "collections.abc.Set",
    "typing.AbstractSet",
    "typing.ByteString",
    "typing.Callable",
    "typing.Collection",
    "typing.Container",
    "typing.FrozenSet",
    "typing.Iterable",
    "typing.Literal",
    "typing.Mapping",
    "typing.Never",
    "typing.NoReturn",
    "typing.Reversible",
    "typing.Sequence",
    "typing.Tuple",
];
// `is_mutable_return_type` de ruff.
const MUTABLE_RETURNS: [&str; 14] = [
    "builtins.dict",
    "builtins.list",
    "builtins.set",
    "collections.ChainMap",
    "collections.Counter",
    "collections.OrderedDict",
    "collections.UserDict",
    "collections.UserList",
    "collections.UserString",
    "collections.defaultdict",
    "collections.deque",
    "weakref.WeakKeyDictionary",
    "weakref.WeakValueDictionary",
    "weakref.WeakSet",
];
// `is_immutable_return_type` de ruff.
const IMMUTABLE_RETURNS: [&str; 25] = [
    "datetime.date",
    "datetime.datetime",
    "datetime.time",
    "datetime.timedelta",
    "datetime.timezone",
    "datetime.tzinfo",
    "decimal.Decimal",
    "fractions.Fraction",
    "operator.attrgetter",
    "operator.itemgetter",
    "operator.methodcaller",
    "pathlib.Path",
    "types.MappingProxyType",
    "re.compile",
    "builtins.bool",
    "builtins.bytes",
    "builtins.complex",
    "builtins.float",
    "builtins.frozenset",
    "builtins.int",
    "builtins.range",
    "builtins.str",
    "builtins.tuple",
    "builtins.slice",
    "builtins.frozendict",
];
const MUTABLE_LITERALS: [&str; 6] = [
    "list",
    "dictionary",
    "set",
    "list_comprehension",
    "dictionary_comprehension",
    "set_comprehension",
];
const TYPING_UNION: &str = "typing.Union";
const TYPING_OPTIONAL: &str = "typing.Optional";
const ANNOTATED: [&str; 2] = ["typing.Annotated", "typing_extensions.Annotated"];
const NEW_TYPE: &str = "NewType";
const NEW_TYPE_KEYWORD: &str = "tp";
const TYPE_NODE: &str = "type";
const GENERIC_TYPE: &str = "generic_type";
const UNION_TYPE: &str = "union_type";
const BINARY: &str = "binary_operator";
const OPERATOR_FIELD: &str = "operator";
const BIT_OR: &str = "|";
const SUBSCRIPT_FIELD: &str = "subscript";

// Resuelve nombres desde `scope`: el actual mientras se recorre, o el del módulo al final del archivo.
pub struct Resolver<'model, 'tree> {
    pub model: &'model Model<'tree>,
    pub source: &'model [u8],
    pub scope: ScopeId,
}

impl Resolver<'_, '_> {
    pub fn qualified(&self, node: Node<'_>) -> Option<String> {
        qualified_name_from(self.model, self.scope, node, self.source)
    }

    // `is_mutable_expr`: una lista, un diccionario o un conjunto (literales o por comprensión), o una
    // llamada a algo que los devuelve.
    pub fn is_mutable_expression(&self, expression: Node<'_>) -> bool {
        let expression = unparenthesized(expression);
        if MUTABLE_LITERALS.contains(&expression.kind()) {
            return true;
        }
        expression.kind() == CALL
            && expression
                .child_by_field_name(FUNCTION_FIELD)
                .is_some_and(|function| self.is_mutable_function(without_subscript(function)))
    }

    pub fn is_mutable_function(&self, function: Node<'_>) -> bool {
        self.qualified(function)
            .is_some_and(|name| MUTABLE_RETURNS.contains(&name.as_str()))
    }

    pub fn is_immutable_function(&self, function: Node<'_>) -> bool {
        self.qualified(without_subscript(function))
            .is_some_and(|name| IMMUTABLE_RETURNS.contains(&name.as_str()))
    }

    pub fn is_immutable_annotation(&self, annotation: Node<'_>) -> bool {
        let annotation = unwrap_type(annotation);
        match annotation.kind() {
            IDENTIFIER | ATTRIBUTE => self.qualified(annotation).is_some_and(|name| {
                IMMUTABLE_NON_GENERIC.contains(&name.as_str()) || IMMUTABLE_GENERIC.contains(&name.as_str())
            }),
            SUBSCRIPT => {
                let mut cursor = annotation.walk();
                let slices: Vec<Node<'_>> = annotation
                    .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
                    .collect();
                annotation
                    .child_by_field_name(VALUE_FIELD)
                    .is_some_and(|value| self.is_immutable_generic(value, &slices))
            }
            GENERIC_TYPE => {
                let parts = named(annotation);
                let slices: Vec<Node<'_>> = parts
                    .iter()
                    .skip(1)
                    .flat_map(|parameters| named(*parameters))
                    .collect();
                parts
                    .first()
                    .is_some_and(|value| self.is_immutable_generic(*value, &slices))
            }
            BINARY => {
                let is_union = annotation
                    .child_by_field_name(OPERATOR_FIELD)
                    .is_some_and(|operator| operator.kind() == BIT_OR);
                is_union
                    && named(annotation)
                        .iter()
                        .all(|side| self.is_immutable_annotation(*side))
            }
            UNION_TYPE => named(annotation)
                .iter()
                .all(|side| self.is_immutable_annotation(*side)),
            NONE => true,
            _ => false,
        }
    }

    // El caso `X[...]` de `is_immutable_annotation`; `slices` son los elementos entre corchetes.
    fn is_immutable_generic(&self, value: Node<'_>, slices: &[Node<'_>]) -> bool {
        let Some(name) = self.qualified(value) else {
            return false;
        };
        let elements = tuple_elements(slices);
        if IMMUTABLE_GENERIC.contains(&name.as_str()) {
            true
        } else if name == TYPING_UNION {
            elements.is_some_and(|elements| {
                elements
                    .iter()
                    .all(|element| self.is_immutable_annotation(*element))
            })
        } else if name == TYPING_OPTIONAL {
            elements.is_none()
                && slices
                    .first()
                    .is_some_and(|slice| self.is_immutable_annotation(*slice))
        } else if ANNOTATED.contains(&name.as_str()) {
            elements.is_some_and(|elements| {
                elements
                    .first()
                    .is_some_and(|first| self.is_immutable_annotation(*first))
            })
        } else {
            false
        }
    }

    // `is_immutable_newtype_call`: un nombre asignado una vez a `NewType("X", tipo_inmutable)`.
    pub fn is_immutable_new_type(&self, name: Node<'_>) -> bool {
        if name.kind() != IDENTIFIER {
            return false;
        }
        let Resolution::Binding(id) = self.model.lookup_from(self.scope, text(name, self.source)) else {
            return false;
        };
        let Some(binding) = self
            .model
            .binding(id)
            .filter(|binding| binding.kind == BindingKind::Assignment)
        else {
            return false;
        };
        let Some(assignment) = binding
            .site
            .statement
            .and_then(|statement| {
                named(statement)
                    .into_iter()
                    .find(|child| child.kind() == ASSIGNMENT)
            })
            .filter(|assignment| assignment.child_by_field_name(TYPE_FIELD).is_none())
        else {
            return false;
        };
        let mut value = assignment.child_by_field_name(RIGHT_FIELD);
        while let Some(chained) = value.filter(|value| value.kind() == ASSIGNMENT) {
            value = chained.child_by_field_name(RIGHT_FIELD);
        }
        value.is_some_and(|call| {
            self.new_type_original(call)
                .is_some_and(|original| self.is_immutable_annotation(original))
        })
    }

    fn new_type_original<'tree>(&self, call: Node<'tree>) -> Option<Node<'tree>> {
        let is_new_type = call.kind() == CALL
            && call
                .child_by_field_name(FUNCTION_FIELD)
                .and_then(|function| self.qualified(function))
                .is_some_and(|name| is_typing(&name, NEW_TYPE));
        if !is_new_type {
            return None;
        }
        let arguments: Vec<Node<'tree>> = named(call.child_by_field_name(ARGUMENTS_FIELD)?)
            .into_iter()
            .filter(|argument| argument.kind() != COMMENT)
            .collect();
        if arguments.len() != 2 {
            return None;
        }
        let keyword = arguments.iter().find(|argument| {
            argument.kind() == KEYWORD_ARGUMENT
                && argument
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|keyword| text(keyword, self.source) == NEW_TYPE_KEYWORD)
        });
        keyword.map_or_else(
            || {
                arguments
                    .get(1)
                    .copied()
                    .filter(|argument| argument.kind() != KEYWORD_ARGUMENT)
            },
            |keyword| keyword.child_by_field_name(VALUE_FIELD),
        )
    }
}

pub fn unwrap_type(node: Node<'_>) -> Node<'_> {
    let node = unparenthesized(node);
    if node.kind() == TYPE_NODE {
        named(node).first().map_or(node, |inner| unwrap_type(*inner))
    } else {
        node
    }
}

// `map_subscript`: `tuple[int]` se resuelve como `tuple`.
pub fn without_subscript(function: Node<'_>) -> Node<'_> {
    if function.kind() == SUBSCRIPT {
        function.child_by_field_name(VALUE_FIELD).unwrap_or(function)
    } else {
        function
    }
}

// Los elementos si lo que hay entre corchetes es una tupla (`X[a, b]` o `X[(a, b)]`).
fn tuple_elements<'tree>(slices: &[Node<'tree>]) -> Option<Vec<Node<'tree>>> {
    match slices {
        [single] => {
            let inner = unwrap_type(*single);
            (inner.kind() == TUPLE).then(|| named(inner).into_iter().map(unwrap_type).collect())
        }
        [] => None,
        many => Some(many.iter().map(|slice| unwrap_type(*slice)).collect()),
    }
}
