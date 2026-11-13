use super::evaluate;
use super::rewrite::parse;
use super::schema::Schema;

fn render(version: &str, schema: Schema, body: &str) -> String {
    let source =
        format!("class Example < ActiveRecord::Migration[{version}]\n  def change\n{body}\n  end\nend\n");
    let tree = parse(&source).expect("el parser devuelve un árbol");
    evaluate(tree.root_node(), source.as_bytes(), schema)
        .map(|(script, _)| script.text().to_owned())
        .unwrap_or_default()
}

fn modern(body: &str) -> String {
    render("7.1", Schema::default(), body)
}

#[test]
fn inlines_locals_and_constants_into_calls_and_heredocs() {
    let sql = modern(concat!(
        "    TABLE = :themes\n",
        "    column = \"color\"\n",
        "    remove_column TABLE, column\n",
        "    execute <<~SQL\n",
        "      UPDATE #{TABLE} SET #{column} = NULL\n",
        "    SQL",
    ));
    for expected in ["DROP COLUMN \"color\"", "UPDATE themes SET color = NULL"] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
}

#[test]
fn keeps_reassigned_locals_and_picks_the_production_branch() {
    let sql = modern(concat!(
        "    column = \"color\"\n",
        "    column = \"layout\" if something?\n",
        "    remove_column :themes, column\n",
        "    execute \"UPDATE themes SET size = #{Rails.env.test? ? 1 : 10}\"",
    ));
    assert!(!sql.contains("DROP COLUMN \"color\""), "{sql}");
    assert!(sql.contains("SET size = 10"), "{sql}");
}

#[test]
fn names_join_tables_without_repeating_the_shared_prefix() {
    let sql = modern("    create_join_table :web_hooks, :web_hook_event_types");
    assert!(
        sql.contains("CREATE TABLE \"web_hook_event_types_hooks\" (\"web_hook_id\" bigint NOT NULL"),
        "{sql}"
    );
}

#[test]
fn indexes_references_only_from_rails_five() {
    let body = "    create_table :posts do |t|\n      t.references :user\n    end";
    assert!(!render("4.2", Schema::default(), body).contains("CREATE INDEX"));
    assert!(render("5.0", Schema::default(), body).contains("CREATE INDEX"));
}

#[test]
fn renames_only_primary_keys_with_a_default() {
    let sql = render(
        "7.1",
        Schema::complete(),
        concat!(
            "    execute \"CREATE TABLE users_search (id integer NOT NULL PRIMARY KEY, data text)\"\n",
            "    rename_table :users_search, :user_search_data",
        ),
    );
    assert!(!sql.contains("users_search_pkey"), "{sql}");
}
