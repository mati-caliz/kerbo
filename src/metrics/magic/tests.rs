use tree_sitter::Parser;

use super::number::numeric_value;
use crate::analysis::analyze;
use crate::language::Language;
use crate::limits::Limits;
use crate::rules::RuleId;

fn magic_numbers(language: Language, source: &str) -> Vec<(usize, String)> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .expect("la gramática carga");
    let limits = Limits::default().with_rule(RuleId::MagicNumber, Some(0));
    analyze(&mut parser, language, source.as_bytes(), limits)
        .expect("el archivo se parsea")
        .magic_numbers
        .into_iter()
        .map(|found| (found.line, found.text))
        .collect()
}

fn found(entries: &[(usize, &str)]) -> Vec<(usize, String)> {
    entries
        .iter()
        .map(|&(line, text)| (line, text.to_owned()))
        .collect()
}

#[test]
fn reads_numbers_in_every_notation() {
    assert_eq!(numeric_value("0x1_0L"), Some(16.0));
    assert_eq!(numeric_value("0b11"), Some(3.0));
    assert_eq!(numeric_value("0o17"), Some(15.0));
    assert_eq!(numeric_value("017"), Some(15.0));
    assert_eq!(numeric_value("2.0f"), Some(2.0));
    assert_eq!(numeric_value("1e3"), Some(1000.0));
    assert_eq!(numeric_value("0x1p3"), None);
}

#[test]
fn flags_numbers_like_eslint_no_magic_numbers_with_the_standard_options() {
    let source = "\
const LIMIT = 50;
export const SIZES = [10, 20];
type Port = 80 | 443;
enum Code { Ok = 200 }
class A { readonly max = 9; open = 7; }
function f(a = 30, list: number[]) {
  const local = 3;
  let total = 5 * 4;
  total = 6;
  this.count = 8;
  const options = { retries: 11 };
  const parsed = parseInt(text, 16) + list[3] + list[-3];
  return -2 + +2 + 2 + 99n + 2n;
}
";
    assert_eq!(
        magic_numbers(Language::TypeScript, source),
        found(&[
            (5, "7"),
            (8, "5"),
            (8, "4"),
            (9, "6"),
            (12, "-3"),
            (13, "-2"),
            (13, "99n"),
            (13, "2n"),
        ]),
        "ni constantes de módulo, ni enums, ni tipos, ni readonly, ni defaults, ni índices, ni el radix"
    );
}

#[test]
fn flags_numbers_like_checkstyle_magic_number() {
    let source = "\
interface Limits { int MAX = 10; }
class A {
  private int timeout = 30;
  static final int[] SIZES = {3, 4};
  @Max(100) int count;
  int compute(int value) {
    final int local = 5 * value;
    final int indexed = values[7];
    final Order order = Order.builder().table(8).build();
    int plain = 6;
    return -2 + 2 + -(9) + 0x10L;
  }
  public int hashCode() { return 31; }
}
enum E { A(5); int z = 7; }
";
    assert_eq!(
        magic_numbers(Language::Java, source),
        found(&[
            (8, "7"),
            (9, "8"),
            (10, "6"),
            (11, "-2"),
            (11, "-(9)"),
            (11, "0x10L"),
            (15, "7")
        ]),
        "los campos de clase, las constantes y hashCode quedan afuera; un índice, una llamada encadenada \
         o un campo de enum, no"
    );
}

#[test]
fn flags_numbers_like_mnd() {
    let source = "\
package a

const timeout = time.Duration(30)

func f(x int) (int, int) {
\ty := T{A: 5, B: 3 * 4}
\tz := x + -6
\tw := 7
\tif x > 3 {
\t}
\tswitch x {
\tcase 8:
\t}
\tt := time.Date(2026, 1, 2, 0, 0, 0, 0, nil)
\tg(9, 2)
\treturn (x * 9), 1
}
";
    assert_eq!(
        magic_numbers(Language::Go, source),
        found(&[
            (6, "5"),
            (6, "3"),
            (6, "4"),
            (7, "6"),
            (9, "3"),
            (12, "8"),
            (15, "9"),
            (15, "2"),
            (16, "9"),
        ]),
        "mnd mira sólo argumentos, asignaciones, casos, condiciones, operaciones y retornos, y no el 2 suelto de w := 7"
    );
}

#[test]
fn flags_numbers_like_ruff_magic_value_comparison() {
    let source = "\
if a > 5 and b == 1 and c < -1 and d != -2:
    pass
if 3 < 4:
    pass
if sys.version_info >= (3, 12) or sys.version_info[0] >= 3:
    pass
if e == 2j or f in 'ab' or g == 0x10 or h == 1.0:
    pass
limit = 50
";
    assert_eq!(
        magic_numbers(Language::Python, source),
        found(&[(1, "5"), (1, "-2"), (7, "2j"), (7, "0x10")]),
        "sólo comparaciones, sin -1, sin literales contra literales y sin sys.version_info"
    );
}

#[test]
fn skips_magic_numbers_when_the_rule_is_off() {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Go.grammar())
        .expect("la gramática carga");
    let analysis = analyze(
        &mut parser,
        Language::Go,
        b"package a\n\nfunc f() int { return 5 }\n",
        Limits::default(),
    )
    .expect("el archivo se parsea");
    assert_eq!(analysis.magic_numbers, Vec::new());
}
