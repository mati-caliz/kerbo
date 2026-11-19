use super::{Builder, LineMode, print};

fn list(items: &[&str]) -> String {
    let mut builder = Builder::default();
    let brackets = builder.group_id();
    builder.token("call");
    builder.start_group(Some(brackets));
    builder.token("(");
    builder.start_indent_if_group_breaks(brackets);
    builder.line(LineMode::Soft);
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            builder.token(",");
            builder.line(LineMode::SoftOrSpace);
        }
        builder.text(item);
    }
    builder.token_if_group_breaks(",");
    builder.end_indent_if_group_breaks();
    builder.line(LineMode::Soft);
    builder.token(")");
    builder.end_group();
    builder.line(LineMode::Hard);
    print(&builder.finish()).expect("el documento es válido")
}

#[test]
fn a_group_that_fits_stays_on_one_line() {
    assert_eq!(list(&["a", "b"]), "call(a, b)\n");
}

#[test]
fn a_group_that_does_not_fit_breaks_every_soft_line_and_indents() {
    let long = "x".repeat(50);
    assert_eq!(
        list(&[&long, &long]),
        format!("call(\n    {long},\n    {long},\n)\n")
    );
}

#[test]
fn the_limit_is_88_columns_and_wide_characters_count_double() {
    let fits = "x".repeat(82);
    assert_eq!(list(&[&fits]), format!("call({fits})\n"));
    let too_long = "x".repeat(83);
    assert_eq!(list(&[&too_long]), format!("call(\n    {too_long},\n)\n"));
    let wide = "字".repeat(42);
    assert_eq!(list(&[&wide]), format!("call(\n    {wide},\n)\n"));
}

#[test]
fn a_hard_line_or_an_expand_parent_breaks_the_enclosing_groups() {
    let mut builder = Builder::default();
    builder.start_group(None);
    builder.token("[");
    builder.start_indent();
    builder.line(LineMode::Soft);
    builder.start_group(None);
    builder.token("a");
    builder.expand_parent();
    builder.line(LineMode::SoftOrSpace);
    builder.token("b");
    builder.end_group();
    builder.end_indent();
    builder.line(LineMode::Soft);
    builder.token("]");
    builder.end_group();
    builder.line(LineMode::Empty);
    builder.text("multi\nline");
    builder.line(LineMode::Hard);
    builder.line(LineMode::Hard);
    assert_eq!(
        print(&builder.finish()).as_deref(),
        Ok("[\n    a\n    b\n]\n\nmulti\nline\n")
    );
}

#[test]
fn a_document_with_an_unbalanced_tag_is_not_printed() {
    let mut builder = Builder::default();
    builder.start_group(None);
    builder.end_indent();
    assert!(print(&builder.finish()).is_err());
    let mut unknown_group = Builder::default();
    let never_opened = unknown_group.group_id();
    unknown_group.start_indent_if_group_breaks(never_opened);
    assert!(print(&unknown_group.finish()).is_err());
}
