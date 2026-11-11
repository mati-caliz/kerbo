use std::fs;
use std::path::Path;

use tree_sitter::Node;

use super::factories::{imported_names, module_file};
use super::zero_downtime::Backend;
use crate::syntax::named_children;

const IMPORT_FROM: &str = "import_from_statement";
const IDENTIFIER: &str = "identifier";
const SAFE_MARKER: &str = "setattr(schema_editor, \"safe\", True)";
const ATTRIBUTE_GUARD_PREFIX: &str = "if self.";
const GUARD_END: char = ':';
const ATOMIC_ATTRIBUTE: &str = "atomic";
const ASSIGNMENT: &str = " = ";
const PYTHON_TRUE: &str = "True";
const PYTHON_FALSE: &str = "False";
const CLASS_KEYWORD: &str = "class ";
const CLASS_ARGUMENTS: char = '(';
const NEWLINE: char = '\n';
const COMMENT_START: char = '#';
const CLOSING_PARENTHESIS: char = ')';

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationBase {
    pub atomic: Option<bool>,
    pub safety: Option<Safety>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Safety {
    pub attribute: Option<String>,
    pub default: bool,
}

pub fn migration_base(
    root: Node<'_>,
    class: Node<'_>,
    source: &[u8],
    apps_root: Option<&Path>,
) -> MigrationBase {
    let (Some(base), Some(apps_root)) = (base_name(class, source), apps_root) else {
        return MigrationBase::default();
    };
    named_children(root)
        .into_iter()
        .filter(|statement| statement.kind() == IMPORT_FROM)
        .flat_map(|statement| imported_names(statement, source))
        .find(|(local, _, _)| *local == base)
        .and_then(|(_, module, imported)| {
            let text = fs::read_to_string(module_file(apps_root, &module)?).ok()?;
            Some(read_base(class_body(&text, &imported)?))
        })
        .unwrap_or_default()
}

impl MigrationBase {
    pub fn zero_downtime(
        &self,
        backend: Option<Backend>,
        own_value: impl Fn(&str) -> Option<bool>,
    ) -> Option<Backend> {
        let safe = self.safety.as_ref().is_some_and(|safety| {
            safety
                .attribute
                .as_deref()
                .and_then(own_value)
                .unwrap_or(safety.default)
        });
        backend.filter(|backend| backend.every_migration || safe)
    }
}

fn base_name(class: Node<'_>, source: &[u8]) -> Option<String> {
    let arguments = class.child_by_field_name("superclasses")?;
    let first = named_children(arguments).into_iter().next()?;
    (first.kind() == IDENTIFIER)
        .then(|| first.utf8_text(source).ok().map(str::to_owned))
        .flatten()
}

fn class_body<'text>(text: &'text str, name: &str) -> Option<&'text str> {
    let header = format!("{CLASS_KEYWORD}{name}{CLASS_ARGUMENTS}");
    let start = text
        .match_indices(&header)
        .map(|(position, _)| position)
        .find(|position| *position == 0 || text[..*position].ends_with(NEWLINE))?;
    let body = &text[start + header.len()..];
    let end = body
        .match_indices(NEWLINE)
        .map(|(position, _)| position + 1)
        .find(|position| {
            body[*position..].chars().next().is_some_and(|character| {
                !character.is_whitespace() && character != COMMENT_START && character != CLOSING_PARENTHESIS
            })
        })
        .unwrap_or(body.len());
    Some(&body[..end])
}

fn read_base(body: &str) -> MigrationBase {
    let lines: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let safety = lines.iter().position(|line| *line == SAFE_MARKER).map(|marker| {
        let attribute = marker
            .checked_sub(1)
            .and_then(|guard| lines.get(guard)?.strip_prefix(ATTRIBUTE_GUARD_PREFIX))
            .and_then(|guard| guard.strip_suffix(GUARD_END))
            .map(str::to_owned);
        let default = attribute
            .as_deref()
            .is_none_or(|name| assigned(&lines, name) == Some(true));
        Safety { attribute, default }
    });
    MigrationBase {
        atomic: assigned(&lines, ATOMIC_ATTRIBUTE),
        safety,
    }
}

fn assigned(lines: &[&str], name: &str) -> Option<bool> {
    lines.iter().find_map(
        |line| match line.strip_prefix(name)?.strip_prefix(ASSIGNMENT)?.trim() {
            PYTHON_TRUE => Some(true),
            PYTHON_FALSE => Some(false),
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECKED: &str = "from django.db.migrations import Migration


class CheckedMigration(Migration):
    \"\"\"Doc.\"\"\"

    # Comentario.
    atomic = False

    checked = True

    def apply(self, project_state, schema_editor, collect_sql=False):
        if self.checked:
            setattr(schema_editor, \"safe\", True)
        return super().apply(project_state, schema_editor, collect_sql)


def validate_operation(op):
    pass
";

    #[test]
    fn reads_the_transaction_and_the_safety_flag_of_the_base() {
        let base = read_base(class_body(CHECKED, "CheckedMigration").expect("la clase existe"));
        assert_eq!(
            base,
            MigrationBase {
                atomic: Some(false),
                safety: Some(Safety {
                    attribute: Some("checked".to_owned()),
                    default: true,
                }),
            }
        );
    }

    #[test]
    fn a_plain_base_marks_nothing() {
        let source = "class Base(Migration):\n    initial = True\n";
        assert_eq!(
            read_base(class_body(source, "Base").expect("la clase existe")),
            MigrationBase::default()
        );
    }
}
