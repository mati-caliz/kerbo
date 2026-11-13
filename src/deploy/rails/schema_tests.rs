use std::fs;

use tree_sitter::Parser;

use super::ddl::index_name;
use super::history::schema_before;
use super::schema::Schema;
use super::sha256::hex_digest;
use super::sql_tokens::{Token, statements};
use super::{evaluate, fingerprint, violations};

fn render_with(schema: Schema, body: &str) -> String {
    let source = format!("class Example < ActiveRecord::Migration[7.1]\n  def change\n{body}\n  end\nend\n");
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_ruby::LANGUAGE.into())
        .expect("la gramática de Ruby carga");
    let tree = parser.parse(&source, None).expect("el parser devuelve un árbol");
    evaluate(tree.root_node(), source.as_bytes(), schema)
        .map(|(script, _)| script.text().to_owned())
        .unwrap_or_default()
}

fn complete(body: &str) -> String {
    render_with(Schema::complete(), body)
}

#[test]
fn hashes_like_sha256() {
    assert_eq!(
        hex_digest(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let columns = ["account_id", "security_id", "date", "currency"].map(String::from);
    assert_eq!(
        index_name("account_holdings", &columns),
        "idx_on_account_id_security_id_date_currency_234024c8e3"
    );
}

#[test]
fn tokenizes_comments_strings_and_dollar_quotes() {
    let parsed = statements("-- nota\nSELECT 'a;b', $$x;y$$, $1 /* c; */ FROM \"T\"; DROP INDEX i");
    assert_eq!(parsed.len(), 2);
    let first = parsed.first().expect("hay una primera sentencia");
    assert!(first.contains(&Token::Quoted("T".to_owned())));
    assert_eq!(first.iter().filter(|token| **token == Token::Literal).count(), 2);
    assert!(first.contains(&Token::Word("$1".to_owned())));
}

#[test]
fn renames_indexes_sequences_and_primary_keys_with_the_table() {
    let sql = complete(concat!(
        "    create_table :holdings do |t|\n",
        "      t.string :name\n",
        "    end\n",
        "    add_index :holdings, :name\n",
        "    rename_table :holdings, :positions\n",
        "    rename_column :positions, :name, :title",
    ));
    for expected in [
        "ALTER INDEX \"holdings_pkey\" RENAME TO \"positions_pkey\";",
        "ALTER TABLE \"holdings_id_seq\" RENAME TO \"positions_id_seq\";",
        "ALTER INDEX \"index_holdings_on_name\" RENAME TO \"index_positions_on_name\";",
        "ALTER INDEX \"index_positions_on_name\" RENAME TO \"index_positions_on_title\";",
    ] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
}

#[test]
fn renaming_an_unknown_table_falls_back_to_the_partial_behaviour() {
    let sql =
        complete("    rename_table :ghosts, :spirits\n    remove_index :spirits, :name, if_exists: true");
    assert!(
        sql.contains("ALTER INDEX \"ghosts_pkey\" RENAME TO \"spirits_pkey\""),
        "{sql}"
    );
    assert!(sql.contains("DROP INDEX \"index_spirits_on_name\""), "{sql}");
}

#[test]
fn skips_drops_of_missing_objects_with_if_exists() {
    let sql = complete(concat!(
        "    execute 'CREATE TABLE posts (id bigint PRIMARY KEY, title text, CONSTRAINT fk_author FOREIGN KEY (author_id) REFERENCES users (id))'\n",
        "    execute 'CREATE INDEX index_posts_on_title ON posts (title)'\n",
        "    execute 'ALTER TABLE posts DROP COLUMN title'\n",
        "    remove_index :posts, :title, if_exists: true\n",
        "    remove_foreign_key :posts, column: :editor_id, if_exists: true\n",
        "    remove_foreign_key :posts, :users, column: :author_id, name: 'fk_author', if_exists: true\n",
        "    remove_index :ghosts, :name, if_exists: true",
    ));
    assert!(!sql.contains("DROP INDEX"), "{sql}");
    assert!(sql.contains("DROP CONSTRAINT \"fk_author\""), "{sql}");
    assert_eq!(sql.matches("DROP CONSTRAINT").count(), 1, "{sql}");
}

#[test]
fn follows_existence_guards() {
    let sql = complete(concat!(
        "    execute 'CREATE TABLE kept (id bigint)'\n",
        "    execute 'CREATE UNIQUE INDEX kept_name ON kept (name)'\n",
        "    drop_table :gone if table_exists?(:gone)\n",
        "    add_index :kept, :name unless index_exists?(:kept, :name)\n",
        "    remove_index :kept, name: 'kept_name' if index_exists?(:kept, name: 'kept_name')\n",
        "    if !foreign_key_exists?(:kept, :users)\n",
        "      add_column :kept, :guarded, :text\n",
        "    else\n",
        "      add_column :kept, :skipped, :text\n",
        "    end\n",
        "    unless (data_source_exists?(:other))\n",
        "      create_table :other\n",
        "    end\n",
        "    add_column :kept, :maybe, :text if ENV['FLAG']\n",
        "    drop_table :kept if connection.table_exists?(:kept)",
    ));
    for expected in [
        "DROP TABLE \"kept\"",
        "DROP INDEX \"kept_name\"",
        "ADD \"guarded\" text",
        "CREATE TABLE \"other\"",
        "ADD \"maybe\" text",
    ] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
    for unexpected in ["\"gone\"", "CREATE INDEX", "\"skipped\""] {
        assert!(!sql.contains(unexpected), "sobra {unexpected} en:\n{sql}");
    }
}

#[test]
fn partial_schemas_keep_the_previous_behaviour() {
    let sql = render_with(
        Schema::default(),
        concat!(
            "    drop_table :legacy if table_exists?(:legacy)\n",
            "    remove_index :legacy, :name, if_exists: true\n",
            "    rename_table :legacy, :archive",
        ),
    );
    for expected in [
        "DROP TABLE \"legacy\"",
        "DROP INDEX \"index_legacy_on_name\"",
        "ALTER INDEX \"legacy_pkey\" RENAME TO \"archive_pkey\"",
    ] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
}

#[test]
fn replays_previous_migrations_of_the_directory() {
    let root = std::env::temp_dir().join(format!("kerbo-rails-history-{}", std::process::id()));
    let directory = root.join("db").join("migrate");
    fs::create_dir_all(&directory).expect("se crea el directorio de migraciones");
    let migration = |body: &str| {
        format!("class Step < ActiveRecord::Migration[7.1]\n  def change\n    {body}\n  end\nend\n")
    };
    let first = directory.join("20240101000000_create.rb");
    let second = directory.join("20240102000000_rename.rb");
    fs::write(&first, migration("create_table :items")).expect("se escribe la primera");
    let source = migration("rename_table :items, :products");
    fs::write(&second, &source).expect("se escribe la segunda");
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_ruby::LANGUAGE.into())
        .expect("la gramática de Ruby carga");
    let tree = parser.parse(&source, None).expect("el parser devuelve un árbol");
    let found = violations(tree.root_node(), source.as_bytes(), Some(&second));
    let known = fingerprint(&second);
    let before = schema_before(&second);
    fs::remove_dir_all(&root).expect("se borra el directorio temporal");
    assert!(known.is_some());
    assert!(fingerprint(&root).is_none());
    assert!(found.iter().any(|violation| violation.line > 0));
    assert_eq!(before.table_exists("items"), Some(true));
    assert_eq!(before.table_exists("products"), Some(false));
}

#[test]
fn reads_word_arrays_in_index_guards() {
    let sql = complete(concat!(
        "    create_table :posts, id: :bigserial
",
        "    add_index :posts, [:a, :b, :c]
",
        "    remove_index :posts, column: [:a, :b] if index_exists?(:posts, %w(a b))
",
        "    remove_index :posts, column: [:a, :b, :c] if index_exists?(:posts, %i(a b c))",
    ));
    assert!(sql.contains("\"id\" bigserial NOT NULL PRIMARY KEY"), "{sql}");
    assert!(!sql.contains("index_posts_on_a_and_b\""), "{sql}");
    assert!(
        sql.contains("DROP INDEX \"index_posts_on_a_and_b_and_c\""),
        "{sql}"
    );
}

#[test]
fn follows_columns_in_existence_guards() {
    let sql = complete(concat!(
        "    create_table :posts do |t|\n",
        "      t.string :title\n",
        "    end\n",
        "    add_column :posts, :body, :text\n",
        "    rename_column :posts, :title, :headline\n",
        "    remove_column :posts, :body\n",
        "    add_column :posts, :kept_headline, :text if column_exists?(:posts, :headline)\n",
        "    add_column :posts, :stale_title, :text if column_exists?(:posts, :title)\n",
        "    add_column :posts, :stale_body, :text if connection.column_exists?(:posts, :body)\n",
        "    add_column :posts, :fresh_body, :text unless column_exists?(:posts, :body)\n",
        "    add_column :posts, :typed, :text if column_exists?(:posts, :id, :bigint)\n",
        "    add_column :ghosts, :never, :text if column_exists?(:ghosts, :id)",
    ));
    for expected in ["\"kept_headline\"", "\"fresh_body\"", "\"typed\""] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
    for unexpected in ["\"stale_title\"", "\"stale_body\"", "\"never\""] {
        assert!(!sql.contains(unexpected), "sobra {unexpected} en:\n{sql}");
    }
}

#[test]
fn columns_of_tables_without_a_definition_stay_unknown() {
    let mut schema = Schema::complete();
    schema.apply("CREATE INDEX \"index_items_on_name\" ON \"items\" (\"name\")");
    schema.apply("CREATE TABLE \"copies\" AS SELECT * FROM \"items\"");
    assert_eq!(schema.column_exists("items", "name"), None);
    assert_eq!(schema.column_exists("copies", "name"), None);
}
