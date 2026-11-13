use super::evaluate;
use super::loops::expand;
use super::rewrite::parse;
use super::schema::Schema;

fn wrapped(body: &str) -> String {
    format!("class Example < ActiveRecord::Migration[7.1]\n  def change\n{body}\n  end\nend\n")
}

fn render(body: &str) -> String {
    render_source(&wrapped(body))
}

fn render_source(source: &str) -> String {
    let tree = parse(source).expect("el parser devuelve un árbol");
    evaluate(tree.root_node(), source.as_bytes(), Schema::default())
        .map(|(script, _)| script.text().to_owned())
        .unwrap_or_default()
}

#[test]
fn unrolls_nested_loops_over_constants_inside_create_table() {
    let sql = render(concat!(
        "    PERIODS = %w[daily weekly].freeze\n",
        "    create_table :stats do |t|\n",
        "      PERIODS.each do |period|\n",
        "        [:likes, :posts].each { |sort| t.integer :\"#{period}_#{sort}_count\" }\n",
        "      end\n",
        "    end",
    ));
    for expected in [
        "\"daily_likes_count\" integer",
        "\"daily_posts_count\" integer",
        "\"weekly_likes_count\" integer",
        "\"weekly_posts_count\" integer",
    ] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
}

#[test]
fn unrolls_word_arrays_and_destructured_heredocs() {
    let sql = render(concat!(
        "    %w[color layout].each { |column| remove_column :themes, column }\n",
        "    [[0, \"a\", \"b\"], [1, \"c\", \"d\"]].each do |position, old, new|\n",
        "      execute <<~SQL\n",
        "        UPDATE themes SET name = '#{new}' WHERE name = '#{old}' AND position = #{position}\n",
        "      SQL\n",
        "    end",
    ));
    for expected in [
        "DROP COLUMN \"color\"",
        "DROP COLUMN \"layout\"",
        "SET name = 'b' WHERE name = 'a' AND position = 0",
        "SET name = 'd' WHERE name = 'c' AND position = 1",
    ] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
}

#[test]
fn keeps_loops_it_cannot_unroll() {
    for body in [
        "    %w[a b].each { |column| next if column == \"a\"; remove_column :themes, column }",
        "    %w[a b].each { |column| column = \"c\"; remove_column :themes, column }",
        "    %w[a b].each { |column| names.each { |column| remove_column :themes, column } }",
        "    names.each { |column| remove_column :themes, column }",
        "    [\"a#b\"].each { |column| remove_column :themes, column }",
    ] {
        let source = wrapped(body);
        let tree = parse(&source).expect("el parser devuelve un árbol");
        assert!(expand(tree.root_node(), source.as_bytes()).is_none(), "{body}");
    }
}

#[test]
fn maps_unrolled_lines_to_the_original_rows() {
    let source = wrapped(
        "    %w[a b].each do |column|\n      remove_column :themes, column\n    end\n    drop_table :old",
    );
    let tree = parse(&source).expect("el parser devuelve un árbol");
    let expansion = expand(tree.root_node(), source.as_bytes()).expect("el bucle se desenrolla");
    let row_of = |needle: &str| {
        expansion
            .source
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains(needle))
            .map(|(row, _)| expansion.rows.get(row).copied())
            .collect::<Vec<_>>()
    };
    assert_eq!(row_of("remove_column"), vec![Some(3), Some(3)]);
    assert_eq!(row_of("drop_table"), vec![Some(5)]);
}

#[test]
fn unrolls_hashes_of_dropped_columns_into_the_dropper_statements() {
    let sql = render(concat!(
        "    DROPPED = { posts: %i[\n      avg_time\n      image_url\n    ], tags: %i[topic_count] }\n",
        "    DROPPED.each { |table, columns| Migration::ColumnDropper.execute_drop(table, columns) }\n",
        "    %i[old_posts].each { |table| Migration::TableDropper.execute_drop(table) }\n",
        "    Migration::ColumnDropper.mark_readonly(:posts, :reply_id)\n",
        "    Migration::TableDropper.read_only_table(\"archived\")\n",
        "    DB.exec(\"DROP INDEX IF EXISTS index_posts_on_raw\")\n",
        "    DB.exec(\"DELETE FROM posts WHERE id = :id AND raw = :raw::text\", id: 1_000, raw: \"it's\")\n",
        "    DB.exec(\"UPDATE posts SET hidden = ? WHERE topic_id = ? AND note = '\\\\?'\", true, nil)\n",
        "    DB.exec(\"DELETE FROM topics WHERE id = :missing\", id: 1)\n",
        "    DB.exec(\"DELETE FROM tags WHERE id = ?\", tag.id)",
    ));
    for expected in [
        "DROP FUNCTION IF EXISTS discourse_functions.raise_posts_avg_time_readonly() CASCADE",
        "DROP FUNCTION IF EXISTS raise_posts_image_url_readonly() CASCADE",
        "ALTER TABLE tags DROP COLUMN IF EXISTS topic_count",
        "DROP TABLE IF EXISTS old_posts",
        "WHEN (NEW.reply_id IS NOT NULL)",
        "CREATE TRIGGER archived_readonly",
        "DROP INDEX IF EXISTS index_posts_on_raw",
    ] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
    assert!(
        sql.contains("DELETE FROM posts WHERE id = 1000 AND raw = 'it''s'::text"),
        "{sql}"
    );
    assert!(
        sql.contains("UPDATE posts SET hidden = TRUE WHERE topic_id = NULL AND note = '?'"),
        "{sql}"
    );
    assert!(!sql.contains("DELETE FROM topics"), "{sql}");
    assert!(!sql.contains("DELETE FROM tags"), "{sql}");
}

#[test]
fn keeps_loops_that_interpolate_a_list() {
    let source = wrapped(
        "    { posts: %i[a b] }.each { |table, columns| execute \"SELECT #{columns} FROM #{table}\" }",
    );
    let tree = parse(&source).expect("el parser devuelve un árbol");
    assert!(expand(tree.root_node(), source.as_bytes()).is_none());
}
