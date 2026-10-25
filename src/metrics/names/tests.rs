use tree_sitter::Parser;

use crate::analysis::analyze;
use crate::language::Language;
use crate::limits::Limits;
use crate::rules::RuleId;

fn short_names(language: Language, source: &str) -> Vec<(usize, String)> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let limits = Limits::default().with_rule(RuleId::ShortName, Some(0));
    analyze(&mut parser, language, source.as_bytes(), &limits)
        .expect("el archivo se parsea")
        .short_names
        .into_iter()
        .map(|found| (found.line, found.name))
        .collect()
}

fn found(entries: &[(usize, &str)]) -> Vec<(usize, String)> {
    entries
        .iter()
        .map(|&(line, name)| (line, name.to_owned()))
        .collect()
}

#[test]
fn flags_names_like_eslint_id_length_without_properties() {
    let source = "\
import d, { x as y, z } from \"m\";
const { a: b, c, e = 1, ...r } = o;
class K extends B { v = w; #p = 1; m() {} constructor(private q: string) {} }
const obj = { s: 1, t() {} };
function f(u, i, _, k = 2) { try {} catch (n) {} }
const g = (h) => h;
for (const x of xs) {}
";
    assert_eq!(
        short_names(Language::TypeScript, source),
        found(&[
            (1, "d"),
            (1, "y"),
            (2, "b"),
            (2, "e"),
            (2, "r"),
            (3, "K"),
            (3, "B"),
            (3, "v"),
            (3, "w"),
            (3, "p"),
            (3, "m"),
            (5, "f"),
            (5, "u"),
            (5, "k"),
            (5, "n"),
            (6, "g"),
            (6, "h"),
            (6, "h"),
            (7, "x"),
        ]),
        "sin propiedades de objeto, sin la propiedad del constructor y con el cuerpo de la flecha"
    );
}

#[test]
fn flags_local_variables_like_checkstyle_local_variable_name() {
    let source = "\
class A {
  int ab;
  void f(int xy) {
    int i = 0;
    int cd = 1;
    final int ef = 2;
    for (int gh = 0; gh < 1; gh++) {}
    for (String s : list) {}
    Runnable r = () -> { int lm = 3; };
  }
}
";
    assert_eq!(
        short_names(Language::Java, source),
        found(&[(5, "cd"), (7, "gh"), (8, "s"), (9, "r"), (9, "lm")]),
        "sólo locales que no son final: ni campos, ni parámetros, ni i"
    );
}

#[test]
fn flags_names_like_varnamelen() {
    let source = "\
package a

func f(t *testing.T, xs []int, ab int) {
\tn := 1
\tfor _, x := range xs {
\t\t_ = x
\t}
\tswitch v := any(n).(type) {
\tcase int:
\t\t_ = v
\t}
\tfoo := n + ab
\t_ = T{n: 1}
\t_ = foo
\t_ = t
}
";
    assert_eq!(
        short_names(Language::Go, source),
        found(&[(3, "ab"), (4, "n")]),
        "cuenta la distancia al último uso; el t de testing es convencional y la clave de un struct no es un uso"
    );
}

#[test]
fn flags_ambiguous_names_like_ruff() {
    let source = "\
l = 1
x.l = 2
a, O = 3, 4
for I in xs:
    pass
def I(l, *O, x=1):
    pass
class O:
    pass
import l
with f() as l:
    pass
";
    assert_eq!(
        short_names(Language::Python, source),
        found(&[
            (1, "l"),
            (3, "O"),
            (4, "I"),
            (6, "I"),
            (6, "l"),
            (6, "O"),
            (8, "O"),
            (11, "l")
        ]),
        "sólo l, O e I, y no los atributos ni los imports"
    );
}
