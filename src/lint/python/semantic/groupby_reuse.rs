use tree_sitter::Node;

use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ALIAS_FIELD, ASSIGNMENT, BLOCK, BODY_FIELD, CALL, CLASS, COMMENT, DECORATED, EXPRESSION_STATEMENT,
    FUNCTION, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, RIGHT_FIELD, TYPE_FIELD, VALUE_FIELD, named, text,
    unparenthesized,
};
use crate::rules::PythonRule;

const GROUPBY: &str = "itertools.groupby";
const GROUPBY_TARGETS: [&str; 2] = ["pattern_list", "tuple_pattern"];
const FOR: &str = "for_statement";
const WHILE: &str = "while_statement";
const IF: &str = "if_statement";
const ELIF: &str = "elif_clause";
const MATCH: &str = "match_statement";
const CASE: &str = "case_clause";
const RETURN: &str = "return_statement";
const LOOP_EXITS: [&str; 2] = ["continue_statement", "break_statement"];
const UNVISITED: [&str; 5] = [
    "import_statement",
    "import_from_statement",
    "future_import_statement",
    "global_statement",
    "nonlocal_statement",
];
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const AS_PATTERN: &str = "as_pattern";
const CLAUSE_SUFFIX: &str = "_clause";
const NAMED_EXPRESSION: &str = "named_expression";
const ELEMENT_COMPREHENSIONS: [&str; 3] = [
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
];
const GENERATOR: &str = "generator_expression";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const DEFAULT_PARAMETER: &str = "default_parameter";
const TYPED_PARAMETERS: [&str; 2] = ["typed_parameter", "typed_default_parameter"];
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const SUBJECT_FIELD: &str = "subject";
const GUARD_FIELD: &str = "guard";
const PARAMETERS_FIELD: &str = "parameters";
const RETURN_TYPE_FIELD: &str = "return_type";
const SUPERCLASSES_FIELD: &str = "superclasses";
const DEFINITION_FIELD: &str = "definition";
const DECORATOR: &str = "decorator";
const PAIR: &str = "pair";

// B031: `for key, group in itertools.groupby(...)` donde el cuerpo usa `group` más de una vez, o
// adentro de un loop: el generador se agota con el primer uso. Avisa en cada uso de más.
pub fn groupby_reuse(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Vec<Report> {
    let Some(group) = groupby_group(model, source, statement) else {
        return Vec::new();
    };
    let mut finder = GroupFinder::new(source, text(group, source));
    if let Some(body) = statement.child_by_field_name(BODY_FIELD) {
        finder.block(body);
    }
    finder
        .found
        .into_iter()
        .map(|usage| Report::at(PythonRule::B031, usage, ""))
        .collect()
}

fn groupby_group<'tree>(model: &Model<'_>, source: &[u8], statement: Node<'tree>) -> Option<Node<'tree>> {
    let target = statement.child_by_field_name(LEFT_FIELD)?;
    if !GROUPBY_TARGETS.contains(&target.kind()) {
        return None;
    }
    let parts = significant(target);
    let [_, group] = parts.as_slice() else {
        return None;
    };
    let iterable = unparenthesized(statement.child_by_field_name(RIGHT_FIELD)?);
    let is_groupby = iterable.kind() == CALL
        && iterable
            .child_by_field_name(FUNCTION_FIELD)
            .and_then(|function| qualified_name(model, function, source))
            .is_some_and(|name| name == GROUPBY);
    (is_groupby && group.kind() == IDENTIFIER).then_some(*group)
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// El `GroupNameFinder` de ruff: cuenta los usos del grupo, por rama en un `if` o un `match` (vale la
// rama que más lo usa), y vuelve a cero con `return`, `break` y `continue`. Asignar el nombre corta
// la búsqueda.
struct GroupFinder<'source, 'tree> {
    source: &'source [u8],
    group: &'source str,
    usage_count: u32,
    nested: bool,
    overridden: bool,
    branches: Vec<Vec<u32>>,
    found: Vec<Node<'tree>>,
}

impl<'source, 'tree> GroupFinder<'source, 'tree> {
    const fn new(source: &'source [u8], group: &'source str) -> Self {
        Self {
            source,
            group,
            usage_count: 0,
            nested: false,
            overridden: false,
            branches: Vec::new(),
            found: Vec::new(),
        }
    }

    fn is_group(&self, node: Node<'_>) -> bool {
        let node = unparenthesized(node);
        node.kind() == IDENTIFIER && text(node, self.source) == self.group
    }

    fn increment(&mut self, value: u32) {
        match self.branches.last_mut().and_then(|branch| branch.last_mut()) {
            Some(count) => *count = count.saturating_add(value),
            None => self.usage_count = self.usage_count.saturating_add(value),
        }
    }

    fn reset(&mut self) {
        match self.branches.last_mut().and_then(|branch| branch.last_mut()) {
            Some(count) => *count = 0,
            None => self.usage_count = 0,
        }
    }

    fn block(&mut self, block: Node<'tree>) {
        for statement in significant(block) {
            self.statement(statement);
        }
    }

    fn nested_block(&mut self, block: Option<Node<'tree>>) {
        self.nested = true;
        if let Some(block) = block {
            self.block(block);
        }
        self.nested = false;
    }

    fn statement(&mut self, statement: Node<'tree>) {
        if self.overridden {
            return;
        }
        match statement.kind() {
            FOR => self.for_statement(statement),
            WHILE => self.nested_block(statement.child_by_field_name(BODY_FIELD)),
            IF => self.if_statement(statement),
            MATCH => self.match_statement(statement),
            EXPRESSION_STATEMENT => {
                for child in significant(statement) {
                    self.statement_part(child);
                }
            }
            kind if LOOP_EXITS.contains(&kind) => self.reset(),
            RETURN => {
                for value in significant(statement) {
                    self.expression(value);
                }
                self.reset();
            }
            DECORATED => self.decorated(statement),
            FUNCTION => self.function(statement),
            CLASS => {
                self.optional(statement.child_by_field_name(SUPERCLASSES_FIELD));
                self.optional_block(statement.child_by_field_name(BODY_FIELD));
            }
            kind if UNVISITED.contains(&kind) => {}
            _ => self.compound(statement),
        }
    }

    fn statement_part(&mut self, part: Node<'tree>) {
        if part.kind() == ASSIGNMENT {
            self.assignment(part);
        } else {
            self.expression(part);
        }
    }

    fn optional(&mut self, node: Option<Node<'tree>>) {
        if let Some(node) = node {
            self.expression(node);
        }
    }

    fn optional_block(&mut self, block: Option<Node<'tree>>) {
        if let Some(block) = block {
            self.block(block);
        }
    }

    // Un `for` o una comprensión que recorre el grupo mismo: cuenta, y avisa si no es el primer uso.
    fn iterated(&mut self, iterable: Option<Node<'tree>>) {
        let Some(iterable) = iterable.filter(|iterable| self.is_group(*iterable)) else {
            return;
        };
        self.increment(1);
        if self.usage_count > 1 {
            self.found.push(unparenthesized(iterable));
        }
    }

    // Un `for` adentro: sólo mira el iterable si es el grupo mismo, y el `else` no cuenta.
    fn for_statement(&mut self, statement: Node<'tree>) {
        if statement
            .child_by_field_name(LEFT_FIELD)
            .is_some_and(|target| self.is_group(target))
        {
            self.overridden = true;
            return;
        }
        self.iterated(statement.child_by_field_name(RIGHT_FIELD));
        self.nested_block(statement.child_by_field_name(BODY_FIELD));
    }

    fn if_statement(&mut self, statement: Node<'tree>) {
        if let Some(condition) = statement.child_by_field_name(CONDITION_FIELD) {
            self.expression(condition);
        }
        self.branches.push(vec![0]);
        if let Some(body) = statement.child_by_field_name(CONSEQUENCE_FIELD) {
            self.block(body);
        }
        let mut cursor = statement.walk();
        let alternatives: Vec<Node<'tree>> = statement
            .children_by_field_name(ALTERNATIVE_FIELD, &mut cursor)
            .collect();
        for alternative in alternatives {
            self.open_branch();
            if alternative.kind() == ELIF {
                self.optional(alternative.child_by_field_name(CONDITION_FIELD));
                self.optional_block(alternative.child_by_field_name(CONSEQUENCE_FIELD));
            } else {
                self.optional_block(alternative.child_by_field_name(BODY_FIELD));
            }
        }
        self.close_branches();
    }

    fn match_statement(&mut self, statement: Node<'tree>) {
        let mut cursor = statement.walk();
        let subjects: Vec<Node<'tree>> = statement
            .children_by_field_name(SUBJECT_FIELD, &mut cursor)
            .collect();
        for subject in subjects {
            self.expression(subject);
        }
        self.branches.push(Vec::new());
        let cases = statement
            .child_by_field_name(BODY_FIELD)
            .map(significant)
            .unwrap_or_default();
        for case in cases.into_iter().filter(|case| case.kind() == CASE) {
            self.open_branch();
            if let Some(guard) = case.child_by_field_name(GUARD_FIELD) {
                self.expression(guard);
            }
            if let Some(body) = case.child_by_field_name(CONSEQUENCE_FIELD) {
                self.block(body);
            }
        }
        self.close_branches();
    }

    fn open_branch(&mut self) {
        if let Some(branch) = self.branches.last_mut() {
            branch.push(0);
        }
    }

    fn close_branches(&mut self) {
        if let Some(branch) = self.branches.pop() {
            self.increment(branch.into_iter().max().unwrap_or(0));
        }
    }

    // `a = b = valor` o `a: T = valor`: si un destino es el grupo, se deja de buscar; si no, sólo se mira
    // el valor.
    fn assignment(&mut self, assignment: Node<'tree>) {
        let mut targets = Vec::new();
        let mut value = Some(assignment);
        while let Some(current) = value.filter(|current| current.kind() == ASSIGNMENT) {
            targets.extend(current.child_by_field_name(LEFT_FIELD));
            value = current.child_by_field_name(RIGHT_FIELD);
            if current.child_by_field_name(TYPE_FIELD).is_some() {
                break;
            }
        }
        if targets.iter().any(|target| self.is_group(*target)) {
            self.overridden = true;
        } else if let Some(value) = value {
            self.expression(value);
        }
    }

    fn decorated(&mut self, statement: Node<'tree>) {
        for decorator in significant(statement)
            .into_iter()
            .filter(|child| child.kind() == DECORATOR)
        {
            self.expression(decorator);
        }
        if let Some(definition) = statement.child_by_field_name(DEFINITION_FIELD) {
            self.statement(definition);
        }
    }

    fn function(&mut self, function: Node<'tree>) {
        if let Some(parameters) = function.child_by_field_name(PARAMETERS_FIELD) {
            self.parameters(parameters);
        }
        if let Some(returns) = function.child_by_field_name(RETURN_TYPE_FIELD) {
            self.expression(returns);
        }
        if let Some(body) = function.child_by_field_name(BODY_FIELD) {
            self.block(body);
        }
    }

    // Los nombres de los parámetros no son expresiones: sólo cuentan los defaults y las anotaciones.
    fn parameters(&mut self, parameters: Node<'tree>) {
        let parts: Vec<Node<'tree>> = significant(parameters)
            .into_iter()
            .filter(|parameter| {
                parameter.kind() == DEFAULT_PARAMETER || TYPED_PARAMETERS.contains(&parameter.kind())
            })
            .flat_map(|parameter| {
                [TYPE_FIELD, VALUE_FIELD]
                    .into_iter()
                    .filter_map(move |field| parameter.child_by_field_name(field))
            })
            .collect();
        for part in parts {
            self.expression(part);
        }
    }

    // `try`, `with`, `assert`, `raise`, `del` y el resto: los bloques como sentencias y todo lo demás
    // como expresiones. El nombre de `except E as e` no es una expresión.
    fn compound(&mut self, statement: Node<'tree>) {
        let handler = EXCEPT_CLAUSES.contains(&statement.kind());
        let alias = statement.child_by_field_name(ALIAS_FIELD);
        for child in significant(statement) {
            if alias.is_some_and(|alias| alias.id() == child.id()) {
                continue;
            }
            if child.kind() == BLOCK {
                self.block(child);
            } else if child.kind().ends_with(CLAUSE_SUFFIX) {
                self.compound(child);
            } else if handler && child.kind() == AS_PATTERN {
                self.optional(significant(child).first().copied());
            } else {
                self.expression(child);
            }
        }
    }
}

mod expressions;
