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
const TYPES_SOURCE: &str = include_str!("fixtures/types.py");
const TYPES_EXPECTED: &str = include_str!("fixtures/types.ruff");
const MATCH_SOURCE: &str = include_str!("fixtures/match.py");
const MATCH_EXPECTED: &str = include_str!("fixtures/match.ruff");
const TYPE_CALLS_SOURCE: &str = include_str!("fixtures/types2.py");
const TYPE_CALLS_EXPECTED: &str = include_str!("fixtures/types2.ruff");
const MATCH_EDGES_SOURCE: &str = include_str!("fixtures/match2.py");
const MATCH_EDGES_EXPECTED: &str = include_str!("fixtures/match2.ruff");
const TSTRINGS_SOURCE: &str = include_str!("fixtures/tstrings.py");
const TSTRINGS_EXPECTED: &str = include_str!("fixtures/tstrings.ruff");
const VERBATIM_SOURCE: &str = include_str!("fixtures/verbatim.py");
const VERBATIM_EXPECTED: &str = include_str!("fixtures/verbatim.ruff");
const FSTRINGS_SOURCE: &str = include_str!("fixtures/fstrings.py");
const FSTRINGS_EXPECTED: &str = include_str!("fixtures/fstrings.ruff");
const FSTRINGS_EDGES_SOURCE: &str = include_str!("fixtures/fstrings2.py");
const FSTRINGS_EDGES_EXPECTED: &str = include_str!("fixtures/fstrings2.ruff");
const CONCATENATION_SOURCE: &str = include_str!("fixtures/concatenation.py");
const CONCATENATION_EXPECTED: &str = include_str!("fixtures/concatenation.ruff");
const CONCATENATION_COMMENT_SOURCE: &str = include_str!("fixtures/concatenation_comment.py");
const CONCATENATION_COMMENT_EXPECTED: &str = include_str!("fixtures/concatenation_comment.ruff");

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

fn assert_formats_once_like_ruff(source: &str, expected: &str) {
    let actual = format(source).expect("el formateador conoce toda la sintaxis del fixture");
    assert!(actual == expected, "{}", first_difference(&actual, expected));
}

fn assert_formats_like_ruff(source: &str, expected: &str) {
    assert_formats_once_like_ruff(source, expected);
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
fn formats_fstrings_like_ruff() {
    assert_formats_like_ruff(FSTRINGS_SOURCE, FSTRINGS_EXPECTED);
}

#[test]
fn formats_fstring_edge_cases_like_ruff() {
    assert_formats_like_ruff(FSTRINGS_EDGES_SOURCE, FSTRINGS_EDGES_EXPECTED);
}

#[test]
fn formats_a_concatenation_with_a_trailing_comment_like_ruff() {
    assert_formats_like_ruff(CONCATENATION_SOURCE, CONCATENATION_EXPECTED);
}

#[test]
fn formats_a_joined_string_with_a_comment_like_ruff() {
    assert_formats_once_like_ruff(CONCATENATION_COMMENT_SOURCE, CONCATENATION_COMMENT_EXPECTED);
}

#[test]
fn formats_type_parameters_and_aliases_like_ruff() {
    assert_formats_like_ruff(TYPES_SOURCE, TYPES_EXPECTED);
}

#[test]
fn formats_match_statements_like_ruff() {
    assert_formats_like_ruff(MATCH_SOURCE, MATCH_EXPECTED);
}

#[test]
fn reads_calls_to_type_like_ruff() {
    assert_formats_like_ruff(TYPE_CALLS_SOURCE, TYPE_CALLS_EXPECTED);
}

#[test]
fn formats_match_edge_cases_like_ruff() {
    assert_formats_like_ruff(MATCH_EDGES_SOURCE, MATCH_EDGES_EXPECTED);
}

#[test]
fn formats_template_strings_like_ruff() {
    assert_formats_like_ruff(TSTRINGS_SOURCE, TSTRINGS_EXPECTED);
}

#[test]
fn leaves_suppressed_statements_as_written_like_ruff() {
    assert_formats_like_ruff(VERBATIM_SOURCE, VERBATIM_EXPECTED);
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
        format("import a\n# fmt: on\npass\n"),
        Err(Unparsable::UnsupportedSyntax {
            line: 2,
            syntax: "comentario que apaga el formateo".to_owned()
        })
    );
    assert_eq!(
        format("pass\n\nx = f'{y := 1}'\n"),
        Err(Unparsable::UnsupportedSyntax {
            line: 3,
            syntax: "interpolation".to_owned()
        })
    );
    assert_eq!(format("import (\n"), Err(Unparsable::SyntaxError { line: 1 }));
    assert_eq!(format("\u{feff}pass\n"), Err(Unparsable::Unsupported { line: 1 }));
}
