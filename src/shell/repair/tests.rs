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
