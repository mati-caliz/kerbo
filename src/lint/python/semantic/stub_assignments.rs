use std::collections::HashSet;

use tree_sitter::Node;

use super::classes::any_base;
use super::model::{MODULE_SCOPE, Model};
use super::mutability::{Resolver, without_subscript};
use super::qualified::{is_typing, qualified_name_from};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, ATTRIBUTE, CALL, CLASS, COMMENT, FUNCTION, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, MODULE,
    RIGHT_FIELD, SUBSCRIPT, TYPE_FIELD, is_f_string, is_string, named, text, unparenthesized,
};
use crate::rules::PythonRule;

const NONE: &str = "none";
const ELLIPSIS: &str = "ellipsis";
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const LITERALS: [&str; 6] = [INTEGER, FLOAT, "true", "false", NONE, ELLIPSIS];
const SEQUENCES: [&str; 3] = ["list", "tuple", "set"];
const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";
const VALUE_FIELD: &str = "value";
const UNARY: &str = "unary_operator";
const BINARY: &str = "binary_operator";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const MINUS: &str = "-";
const PLUS: &str = "+";
const BIT_OR: &str = "|";
const COMPLEX_SUFFIXES: [char; 2] = ['j', 'J'];
const MAX_CONTAINER_ITEMS: usize = 10;
const MAX_NUMBER_LENGTH: usize = 10;
const NEGATABLE_CONSTANTS: [&str; 4] = ["math.inf", "math.e", "math.pi", "math.tau"];
const CONSTANTS: [&str; 19] = [
    "math.inf",
    "math.nan",
    "math.e",
    "math.pi",
    "math.tau",
    "sys.stdin",
    "sys.stdout",
    "sys.stderr",
    "sys.version",
    "sys.version_info",
    "sys.platform",
    "sys.executable",
    "sys.prefix",
    "sys.exec_prefix",
    "sys.base_prefix",
    "sys.byteorder",
    "sys.maxsize",
    "sys.hexversion",
    "sys.winver",
];
const TYPE_VARIABLES: [&str; 3] = ["TypeVar", "ParamSpec", "TypeVarTuple"];
const TYPE_VARIABLE_LIKE: [&str; 4] = ["TypeVar", "TypeVarTuple", "NewType", "ParamSpec"];
const MODULE_SPECIAL: &str = "__all__";
const CLASS_SPECIALS: [&str; 2] = ["__match_args__", "__slots__"];
const TYPE_ALIAS: &str = "TypeAlias";
const FINAL: &str = "Final";
const ANY: &str = "Any";
const PRIVATE_PREFIX: char = '_';
const ENUMS: [&str; 7] = [
    "enum.Enum",
    "enum.IntEnum",
    "enum.StrEnum",
    "enum.Flag",
    "enum.IntFlag",
    "enum.ReprEnum",
    "enum.EnumCheck",
];

// Las asignaciones de un stub fuera de las funciones: PYI001 (`TypeVar` sin `_`), PYI015 (valor que no
// es simple), PYI052 (valor simple sin anotación), PYI017 (varios destinos o un atributo), PYI026
// (alias de tipo sin `TypeAlias`) y PYI035 (`__all__`, `__slots__` o `__match_args__` sin valor).
pub fn stub_assignment_reports(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Vec<Report> {
    let Some(assignment) = named(statement)
        .into_iter()
        .find(|child| child.kind() != COMMENT)
        .filter(|child| child.kind() == ASSIGNMENT)
    else {
        return Vec::new();
    };
    let Some(owner) = std::iter::successors(statement.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS | MODULE))
        .filter(|owner| owner.kind() != FUNCTION)
    else {
        return Vec::new();
    };
    let context = Context {
        resolver: Resolver {
            model,
            source,
            scope: MODULE_SCOPE,
        },
        owner,
    };
    let (targets, value) = chain(assignment);
    let target = match targets.as_slice() {
        [only] => Some(*only).filter(|target| target.kind() == IDENTIFIER),
        _ => None,
    };
    if assignment.child_by_field_name(TYPE_FIELD).is_some() {
        return context.annotated(statement, assignment, target);
    }
    let Some(value) = value.map(unparenthesized) else {
        return Vec::new();
    };
    let Some(target) = target else {
        return vec![Report::at(PythonRule::PYI017, statement, "")];
    };
    context.plain(target, value)
}

struct Context<'model, 'tree> {
    resolver: Resolver<'model, 'tree>,
    owner: Node<'tree>,
}

impl<'model> Context<'model, '_> {
    fn qualified(&self, node: Node<'_>) -> Option<String> {
        qualified_name_from(self.resolver.model, MODULE_SCOPE, node, self.resolver.source)
    }

    fn text(&self, node: Node<'_>) -> &'model str {
        text(node, self.resolver.source)
    }

    fn plain(&self, target: Node<'_>, value: Node<'_>) -> Vec<Report> {
        let mut reports = Vec::new();
        if !self.text(target).starts_with(PRIVATE_PREFIX) && self.is_call_to(value, &TYPE_VARIABLES) {
            reports.push(Report::at(PythonRule::PYI001, value, ""));
        }
        let ignored = self.is_special(target) || self.is_call_to(value, &TYPE_VARIABLE_LIKE);
        if !ignored && !self.is_valid_without_annotation(value) {
            if !self.is_valid_with_annotation(value, true) {
                reports.push(Report::at(PythonRule::PYI015, value, ""));
            } else if !self.in_enum() {
                reports.push(Report::at(PythonRule::PYI052, value, self.text(target)));
            }
        }
        if self.is_annotatable_type_alias(value) {
            reports.push(Report::at(PythonRule::PYI026, target, self.text(target)));
        }
        reports
    }

    fn annotated(&self, statement: Node<'_>, assignment: Node<'_>, target: Option<Node<'_>>) -> Vec<Report> {
        let Some(annotation) = assignment.child_by_field_name(TYPE_FIELD) else {
            return Vec::new();
        };
        let (_, value) = chain(assignment);
        let special = target.is_some_and(|target| self.is_special(target));
        let Some(value) = value.map(unparenthesized) else {
            return if special {
                vec![Report::at(PythonRule::PYI035, statement, "")]
            } else {
                Vec::new()
            };
        };
        let annotation = without_subscript(unparenthesized(
            named(annotation).first().copied().unwrap_or(annotation),
        ));
        let is_typing_named = |member: &str| {
            self.qualified(annotation)
                .is_some_and(|name| is_typing(&name, member))
        };
        let is_final = matches!(value.kind(), IDENTIFIER | ATTRIBUTE) && is_typing_named(FINAL);
        let skipped = is_typing_named(TYPE_ALIAS)
            || special
            || self.is_call_to(value, &TYPE_VARIABLE_LIKE)
            || is_final
            || self.is_valid_with_annotation(value, true);
        if skipped {
            Vec::new()
        } else {
            vec![Report::at(PythonRule::PYI015, value, "")]
        }
    }

    fn is_special(&self, target: Node<'_>) -> bool {
        let name = self.text(target);
        match self.owner.kind() {
            MODULE => name == MODULE_SPECIAL,
            CLASS => CLASS_SPECIALS.contains(&name),
            _ => false,
        }
    }

    fn is_call_to(&self, value: Node<'_>, members: &[&str]) -> bool {
        value.kind() == CALL
            && value
                .child_by_field_name(FUNCTION_FIELD)
                .and_then(|function| self.qualified(function))
                .is_some_and(|name| members.iter().any(|member| is_typing(&name, member)))
    }

    fn in_enum(&self) -> bool {
        self.owner.kind() == CLASS
            && any_base(
                &self.resolver,
                self.owner,
                &mut |base| {
                    self.qualified(without_subscript(base))
                        .is_some_and(|name| ENUMS.contains(&name.as_str()))
                },
                &mut HashSet::new(),
            )
    }

    fn is_annotatable_type_alias(&self, value: Node<'_>) -> bool {
        if value.kind() == NONE {
            return !self.in_enum();
        }
        value.kind() == SUBSCRIPT
            || self.is_pep604_union(value)
            || self.qualified(value).is_some_and(|name| is_typing(&name, ANY))
    }

    fn is_valid_without_annotation(&self, value: Node<'_>) -> bool {
        matches!(
            value.kind(),
            CALL | IDENTIFIER | ATTRIBUTE | SUBSCRIPT | ELLIPSIS | NONE
        ) || self.is_pep604_union(value)
    }

    fn is_pep604_union(&self, value: Node<'_>) -> bool {
        self.is_bit_or(value)
            && [LEFT_FIELD, RIGHT_FIELD]
                .into_iter()
                .filter_map(|side| value.child_by_field_name(side))
                .all(|side| self.is_union_member(unparenthesized(side)))
    }

    fn is_union_member(&self, member: Node<'_>) -> bool {
        matches!(member.kind(), IDENTIFIER | SUBSCRIPT | ATTRIBUTE | NONE) || self.is_pep604_union(member)
    }

    fn is_bit_or(&self, value: Node<'_>) -> bool {
        self.binary_operator(value) == Some(BIT_OR)
    }

    fn binary_operator(&self, value: Node<'_>) -> Option<&'model str> {
        (value.kind() == BINARY)
            .then(|| value.child_by_field_name(OPERATOR_FIELD))
            .flatten()
            .map(|operator| self.text(operator))
    }

    // `is_valid_default_value_with_annotation` de ruff.
    fn is_valid_with_annotation(&self, value: Node<'_>, allow_container: bool) -> bool {
        let value = unparenthesized(value);
        let elements = || -> Vec<Node<'_>> {
            named(value)
                .into_iter()
                .filter(|child| child.kind() != COMMENT)
                .collect()
        };
        match value.kind() {
            kind if LITERALS.contains(&kind) => true,
            _ if is_string(value) => !is_f_string(value, self.resolver.source),
            kind if SEQUENCES.contains(&kind) => {
                let items = elements();
                allow_container
                    && items.len() <= MAX_CONTAINER_ITEMS
                    && items
                        .iter()
                        .all(|item| self.is_valid_with_annotation(*item, false))
            }
            DICTIONARY => {
                let items = elements();
                allow_container
                    && items.len() <= MAX_CONTAINER_ITEMS
                    && items.iter().all(|item| self.is_valid_pair(*item))
            }
            UNARY => self.is_negated_constant(value),
            BINARY => self.is_complex_literal(value),
            ATTRIBUTE => self
                .qualified(value)
                .is_some_and(|name| CONSTANTS.contains(&name.as_str())),
            _ => false,
        }
    }

    fn is_valid_pair(&self, item: Node<'_>) -> bool {
        item.kind() == PAIR
            && [KEY_FIELD, VALUE_FIELD].into_iter().all(|field| {
                item.child_by_field_name(field)
                    .is_some_and(|part| self.is_valid_with_annotation(part, false))
            })
    }

    fn is_negated_constant(&self, value: Node<'_>) -> bool {
        let Some(operand) = self.negated(value) else {
            return false;
        };
        matches!(operand.kind(), INTEGER | FLOAT)
            || (operand.kind() == ATTRIBUTE
                && self
                    .qualified(operand)
                    .is_some_and(|name| NEGATABLE_CONSTANTS.contains(&name.as_str())))
    }

    fn negated<'tree>(&self, value: Node<'tree>) -> Option<Node<'tree>> {
        let operator = value.child_by_field_name(OPERATOR_FIELD)?;
        (value.kind() == UNARY && self.text(operator) == MINUS)
            .then(|| value.child_by_field_name(ARGUMENT_FIELD))
            .flatten()
            .map(unparenthesized)
    }

    // `1 + 2j` o `-1 - 2j`, con la parte real de diez caracteres como mucho.
    fn is_complex_literal(&self, value: Node<'_>) -> bool {
        if !matches!(self.binary_operator(value), Some(PLUS | MINUS)) {
            return false;
        }
        let (Some(left), Some(right)) = (
            value.child_by_field_name(LEFT_FIELD).map(unparenthesized),
            value.child_by_field_name(RIGHT_FIELD).map(unparenthesized),
        ) else {
            return false;
        };
        if !self.is_complex_number(right) {
            return false;
        }
        let real = self.negated(left).unwrap_or(left);
        matches!(real.kind(), INTEGER | FLOAT)
            && !self.is_complex_number(real)
            && self.text(real).len() <= MAX_NUMBER_LENGTH
    }

    fn is_complex_number(&self, number: Node<'_>) -> bool {
        matches!(number.kind(), INTEGER | FLOAT) && self.text(number).ends_with(COMPLEX_SUFFIXES)
    }
}

// Los destinos de `a = b = valor` y el valor.
fn chain(assignment: Node<'_>) -> (Vec<Node<'_>>, Option<Node<'_>>) {
    let mut targets = Vec::new();
    let mut current = assignment;
    loop {
        targets.extend(current.child_by_field_name(LEFT_FIELD).map(unparenthesized));
        match current.child_by_field_name(RIGHT_FIELD) {
            Some(next) if next.kind() == ASSIGNMENT => current = next,
            value => return (targets, value),
        }
    }
}
