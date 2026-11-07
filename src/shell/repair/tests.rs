use super::blank_backtick_comments;

fn blanked(source: &str) -> Option<String> {
    blank_backtick_comments(source.as_bytes()).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

#[test]
fn a_backtick_comment_turns_into_spaces_of_the_same_length() {
    assert_eq!(
        blanked("git ls-files 'a' `# nota` \\\n  'b' `#otra`\n").as_deref(),
        Some("git ls-files 'a'          \\\n  'b'        \n")
    );
}

#[test]
fn only_a_whole_word_that_is_a_comment_is_blanked() {
    assert_eq!(blanked("a=`date`\n"), None);
    assert_eq!(blanked("echo x`# nota`\n"), None);
    assert_eq!(blanked("echo `# nota`y\n"), None);
    assert_eq!(blanked("echo `# sin cerrar\n"), None);
}

#[test]
fn a_line_starting_with_a_backslash_gets_a_space_in_front() {
    let source = b"set +e # nota\n\\. ./a.sh\n";
    let repaired = super::indent_lines(source, &[14]);
    assert_eq!(repaired.source, b"set +e # nota\n \\. ./a.sh\n");
    assert_eq!(repaired.origin.get(14..17), Some(&[14, 14, 15][..]));
}
