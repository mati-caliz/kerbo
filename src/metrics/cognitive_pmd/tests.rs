use tree_sitter::Parser;

use crate::analysis::analyze;
use crate::language::Language;
use crate::limits::Limits;

fn cognitive_by_line(source: &str) -> Vec<(usize, Option<usize>)> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Java.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    analyze(&mut parser, Language::Java, source.as_bytes(), Limits::default())
        .expect("el archivo se parsea")
        .functions
        .into_iter()
        .map(|function| (function.line, function.cognitive))
        .collect()
}

#[test]
fn resolves_recursion_and_boolean_runs_like_pmd() {
    let source = "\
class Node {
  Node parent;
  Node(int port, int flags) {
    check(port >= 0 && port <= 9);
    check(flags >= 0 && flags <= 9);
  }
  void climb() {
    parent.climb();
  }
  Node self() { return this; }
  void walk(int depth, int limit) {
    self().walk(depth /* siguiente */, limit);
  }
  static Set<E> wrap(Set<E> set) {
    return new Forwarding<E>(set) {
      Set<E> head(E element) {
        return wrap(super.head(element));
      }
    };
  }
}
class Base {
  String join(Iterable<?> parts) { return \"\"; }
  String join(Iterator<?> parts) { return \"\"; }
}
class Child extends Base {
  String join(Iterable<?> parts) {
    return join(parts.iterator());
  }
}
";
    assert_eq!(
        cognitive_by_line(source),
        [
            (3, Some(2)),
            (7, Some(1)),
            (10, Some(0)),
            (11, Some(1)),
            (14, Some(1)),
            (16, Some(1)),
            (23, Some(0)),
            (24, Some(0)),
            (27, Some(0)),
        ],
        "el cuerpo del constructor corta la secuencia booleana, la recursión sigue un campo de la propia \\
         clase y el método que envuelve a la clase anónima, y una sobrecarga heredada no es recursión"
    );
}
