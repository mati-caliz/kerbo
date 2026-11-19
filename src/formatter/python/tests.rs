use super::format;
use crate::formatter::Unparsable;

const STATEMENTS_SOURCE: &str = include_str!("fixtures/statements.py");
const STATEMENTS_EXPECTED: &str = include_str!("fixtures/statements.ruff");
const EXPRESSIONS_SOURCE: &str = include_str!("fixtures/expressions.py");
const EXPRESSIONS_EXPECTED: &str = include_str!("fixtures/expressions.ruff");
const COMMENTS_SOURCE: &str = include_str!("fixtures/comments.py");
const COMMENTS_EXPECTED: &str = include_str!("fixtures/comments.ruff");
const STRINGS_SOURCE: &str = include_str!("fixtures/strings.py");
const STRINGS_EXPECTED: &str = include_str!("fixtures/strings.ruff");

fn first_difference(actual: &str, expected: &str) -> String {
    let mismatch = actual
        .lines()
        .zip(expected.lines())
        .enumerate()
        .find(|(_, (actual_line, expected_line))| actual_line != expected_line);
    match mismatch {
        Some((index, (actual_line, expected_line))) => {
            format!(
                "línea {}:\n  kerbo: {actual_line}\n  ruff:  {expected_line}",
                index + 1
            )
        }
        None => format!(
            "kerbo escribió {} líneas y ruff {}",
            actual.lines().count(),
            expected.lines().count()
        ),
    }
}

fn assert_formats_like_ruff(source: &str, expected: &str) {
    let actual = format(source).expect("el formateador conoce toda la sintaxis del fixture");
    assert!(actual == expected, "{}", first_difference(&actual, expected));
    assert_eq!(
        format(expected).as_deref(),
        Ok(expected),
        "formatear dos veces cambia"
    );
}

#[test]
fn formats_statements_like_ruff() {
    assert_formats_like_ruff(STATEMENTS_SOURCE, STATEMENTS_EXPECTED);
}

#[test]
fn formats_expressions_like_ruff() {
    assert_formats_like_ruff(EXPRESSIONS_SOURCE, EXPRESSIONS_EXPECTED);
}

#[test]
fn formats_comments_like_ruff() {
    assert_formats_like_ruff(COMMENTS_SOURCE, COMMENTS_EXPECTED);
}

#[test]
fn formats_annotations_docstrings_strings_and_lambdas_like_ruff() {
    assert_formats_like_ruff(STRINGS_SOURCE, STRINGS_EXPECTED);
}

#[test]
fn keeps_the_line_ending_of_the_first_line() {
    assert_formats_like_ruff(
        "import a\r\nimport   b\r\n\r\n\r\n\r\n\r\npass",
        "import a\r\nimport b\r\n\r\n\r\npass\r\n",
    );
}

#[test]
fn an_empty_file_stays_empty_and_blank_lines_become_one() {
    assert_formats_like_ruff("", "");
    assert_formats_like_ruff("  ", "");
    assert_eq!(format("\n\n  \n").as_deref(), Ok("\n"));
}

#[test]
fn leaves_untouched_what_is_not_ported_yet() {
    assert_eq!(
        format("import a\n# fmt: off\n"),
        Err(Unparsable::UnsupportedSyntax {
            line: 2,
            syntax: "comentario que apaga el formateo".to_owned()
        })
    );
    assert_eq!(
        format("pass\n\nx = f'{y}'\n"),
        Err(Unparsable::UnsupportedSyntax {
            line: 3,
            syntax: "string".to_owned()
        })
    );
    assert_eq!(format("import (\n"), Err(Unparsable::SyntaxError { line: 1 }));
    assert_eq!(format("\u{feff}pass\n"), Err(Unparsable::Unsupported { line: 1 }));
}
