use tree_sitter::Parser;

use super::find_commented_code;
use crate::language::Language;
use crate::profile::CommentedCodeStyle;

fn commented_lines(source: &str) -> Vec<usize> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar())
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    find_commented_code(tree.root_node(), source.as_bytes(), CommentedCodeStyle::Eradicate)
        .into_iter()
        .map(|found| found.line)
        .collect()
}

#[test]
fn flags_own_line_comments_that_compile_like_ruff() {
    let source = "# x = 1
# # x = 1
#from foo import eradicate
#\"key\": value,
#}
# This is a (real) comment.
# 123
# 1, 2, 3
# pylint: disable=redefined-outer-name
# Issue #999: This is not code
# TODO(tom): x = 1
y = 2  # z = 3
#print(1)
#print 1
#print \"x\"
# else: print()
# return x
";
    assert_eq!(commented_lines(source), vec![1, 2, 3, 4, 5, 13, 16, 17]);
}

#[test]
fn skips_inline_script_metadata() {
    let source = "# /// script
# dependencies = [
#   \"requests<3\",
# ]
# ///
# x = 1
";
    assert_eq!(commented_lines(source), vec![6]);
}

#[test]
fn other_languages_are_not_measured_yet() {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar())
        .expect("la gramática carga");
    let tree = parser.parse("# x = 1\n", None).expect("el archivo se parsea");
    assert_eq!(
        find_commented_code(tree.root_node(), b"# x = 1\n", CommentedCodeStyle::Unmeasured),
        Vec::new()
    );
}
