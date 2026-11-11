use std::fs;
use std::path::Path;

use tree_sitter::{Node, Parser};

use super::constants::assignments;
use super::factories::module_file;
use super::values::{sql_literal, text};
use crate::language::Language;
use crate::syntax::named_children;

const SUBSCRIPT: &str = "subscript";
const CLASS_DEFINITION: &str = "class_definition";
const DECORATED_DEFINITION: &str = "decorated_definition";
const PATH_SEPARATOR: char = '.';

pub fn member_literal(node: Node<'_>, source: &[u8], apps_root: Option<&Path>) -> Option<String> {
    if node.kind() != SUBSCRIPT {
        return None;
    }
    let path = node.child_by_field_name("value")?.utf8_text(source).ok()?;
    let member = text(node.child_by_field_name("subscript")?, source)?;
    let segments: Vec<&str> = path.split(PATH_SEPARATOR).collect();
    (1..segments.len()).rev().find_map(|split| {
        let (module, classes) = segments.split_at(split);
        let file = module_file(apps_root?, &module.join(&PATH_SEPARATOR.to_string()))?;
        member_in_file(&file, classes, &member)
    })
}

fn member_in_file(file: &Path, classes: &[&str], member: &str) -> Option<String> {
    let source = fs::read(file).ok()?;
    let mut parser = Parser::new();
    parser.set_language(&Language::Python.grammar()?).ok()?;
    let tree = parser.parse(&source, None)?;
    let body = classes
        .iter()
        .try_fold(tree.root_node(), |scope, class| class_body(scope, class, &source))?;
    let value = assignments(body, &source)
        .into_iter()
        .find(|(name, _)| name == member)?
        .1;
    sql_literal(value, &source)
}

fn class_body<'tree>(scope: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    named_children(scope)
        .into_iter()
        .filter_map(undecorated)
        .filter(|node| node.kind() == CLASS_DEFINITION)
        .find(|class| {
            class
                .child_by_field_name("name")
                .and_then(|defined| defined.utf8_text(source).ok())
                == Some(name)
        })?
        .child_by_field_name("body")
}

fn undecorated(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == DECORATED_DEFINITION {
        node.child_by_field_name("definition")
    } else {
        Some(node)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::member_literal;
    use crate::deploy::django::tests::parsed;

    #[test]
    fn enum_members_of_project_modules_render_their_value() {
        let root = std::env::temp_dir().join("kerbo-enum-members");
        let package = root.join("billing");
        fs::create_dir_all(&package).expect("se crea el paquete");
        fs::write(
            package.join("constants.py"),
            "@registered
class Plan(Base):\n    scope = 1\n\n    class Mode(enum.StrEnum):\n        # activo\n        ACTIVE = \"active\"\n",
        )
        .expect("se escribe el módulo");
        let source = "billing.constants.Plan.Mode[\n    \"ACTIVE\"\n]\n";
        let tree = parsed(source);
        let subscript = tree
            .root_node()
            .named_child(0)
            .and_then(|statement| statement.named_child(0))
            .expect("hay una expresión");
        assert_eq!(
            member_literal(subscript, source.as_bytes(), Some(&root)).as_deref(),
            Some("'active'")
        );
        assert_eq!(member_literal(subscript, source.as_bytes(), None), None);
    }
}
