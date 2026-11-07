//! Imprime dónde la gramática de bash de kerbo no lee cada script: los nodos de error y los que
//! faltan, con línea, columna y el comienzo del texto. Lo usa `scripts/parity/shell/parse_errors.sh`.

use std::fs;

use tree_sitter::{Node, Parser};

const SHOWN_ERRORS: usize = 5;
const SHOWN_CHARACTERS: usize = 70;

fn main() {
    let mut parser = Parser::new();
    if parser.set_language(&tree_sitter_bash::LANGUAGE.into()).is_err() {
        eprintln!("no se pudo cargar la gramática de bash");
        return;
    }
    for path in std::env::args().skip(1) {
        let Ok(source) = fs::read(&path) else {
            eprintln!("{path}: no se pudo leer");
            continue;
        };
        let Some(tree) = parser.parse(&source, None) else {
            println!("{path}: sin árbol");
            continue;
        };
        let errors = error_nodes(tree.root_node());
        let swallowed = swallowing_commands(tree.root_node(), &source);
        if errors.is_empty() && swallowed.is_empty() {
            continue;
        }
        println!(
            "{path}: {} errores, {} comandos que se tragan líneas",
            errors.len(),
            swallowed.len()
        );
        for node in errors.into_iter().take(SHOWN_ERRORS) {
            let kind = if node.is_missing() { "falta" } else { "error" };
            show(node, &source, &format!("{kind} {}", node.kind()));
        }
        for node in swallowed.into_iter().take(SHOWN_ERRORS) {
            show(node, &source, "se traga");
        }
    }
}

fn show(node: Node<'_>, source: &[u8], label: &str) {
    let start = node.start_position();
    let line = source
        .split(|byte| *byte == b'\n')
        .nth(start.row)
        .map(String::from_utf8_lossy)
        .unwrap_or_default();
    let shown: String = line.chars().take(SHOWN_CHARACTERS).collect();
    println!("  {}:{} {label}| {shown}", start.row + 1, start.column + 1);
}

// Un comando con una palabra que empieza en otra línea sin una `\` que la continúe.
fn swallowing_commands<'tree>(root: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let mut cursor = node.walk();
        let children: Vec<Node<'tree>> = node.children(&mut cursor).collect();
        if node.kind() == "command" && children.windows(2).any(|pair| splits_lines(pair, source)) {
            found.push(node);
        }
        pending.extend(children);
    }
    found.sort_by_key(Node::start_byte);
    found
}

fn splits_lines(pair: &[Node<'_>], source: &[u8]) -> bool {
    let [before, after] = pair else {
        return false;
    };
    let gap = source
        .get(before.end_byte()..after.start_byte())
        .unwrap_or_default();
    after.kind() != "comment" && gap.contains(&b'\n') && !gap.windows(2).any(|window| window == b"\\\n")
}

fn error_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.is_error() || node.is_missing() {
            found.push(node);
            continue;
        }
        if node.has_error() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
        }
    }
    found.sort_by_key(Node::start_byte);
    found
}
