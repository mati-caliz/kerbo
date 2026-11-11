use std::path::Path;

use tree_sitter::Parser;

use super::{is_migration, script};

fn render(source: &str) -> String {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_ruby::LANGUAGE.into())
        .expect("la gramática de Ruby carga");
    let tree = parser.parse(source, None).expect("el parser devuelve un árbol");
    script(tree.root_node(), source.as_bytes())
        .map(|script| script.text().to_owned())
        .unwrap_or_default()
}

fn migration(body: &str) -> String {
    render(&format!(
        "class Example < ActiveRecord::Migration[7.1]\n  def change\n{body}\n  end\nend\n"
    ))
}

#[test]
fn detects_migrations_under_db_migrate() {
    assert!(is_migration(Path::new("app/db/migrate/20240101000000_add.rb")));
    assert!(!is_migration(Path::new("app/models/user.rb")));
}

#[test]
fn ignores_files_without_a_migration_class() {
    assert_eq!(render("class User < ApplicationRecord\nend\n"), "");
}

#[test]
fn renders_create_table_with_columns_timestamps_and_references() {
    let sql = migration(
        "    create_table :posts do |t|\n      t.string :title, null: false\n      t.references :user, foreign_key: true\n      t.timestamps\n    end",
    );
    assert!(sql.starts_with("BEGIN;\nCREATE TABLE \"posts\" (\"id\" bigserial primary key, \"title\" character varying NOT NULL, \"user_id\" bigint,"));
    assert!(sql.contains("\"created_at\" timestamp(6) NOT NULL"));
    assert!(sql.contains("REFERENCES \"users\" (\"id\")"));
    assert!(sql.contains("CREATE INDEX \"index_posts_on_user_id\" ON \"posts\" (\"user_id\")"));
    assert!(sql.ends_with("COMMIT;\n"));
}

#[test]
fn renders_column_and_index_changes() {
    let sql = migration(
        "    add_column :users, :age, :integer, default: 0, null: false\n    add_index :users, [:email, :age], unique: true, algorithm: :concurrently\n    remove_column :users, :legacy",
    );
    assert!(sql.contains("ALTER TABLE \"users\" ADD \"age\" integer DEFAULT 0 NOT NULL;"));
    assert!(sql.contains(
        "CREATE UNIQUE INDEX CONCURRENTLY \"index_users_on_email_and_age\" ON \"users\" (\"email\", \"age\");"
    ));
    assert!(sql.contains("ALTER TABLE \"users\" DROP COLUMN \"legacy\";"));
}

#[test]
fn skips_the_transaction_when_disabled() {
    let sql = render(
        "class Example < ActiveRecord::Migration[7.1]\n  disable_ddl_transaction!\n  def change\n    add_index :users, :email, algorithm: :concurrently\n  end\nend\n",
    );
    assert!(sql.starts_with("COMMIT;\nCREATE INDEX CONCURRENTLY"));
    assert!(!sql.contains("BEGIN"));
}

#[test]
fn renders_execute_heredocs_and_reversible_up_blocks() {
    let sql = migration(
        "    reversible do |dir|\n      dir.up do\n        execute <<~SQL\n          UPDATE users SET age = 1\n        SQL\n      end\n      dir.down { execute \"DELETE FROM users\" }\n    end",
    );
    assert!(sql.contains("UPDATE users SET age = 1"));
    assert!(!sql.contains("DELETE"));
}

#[test]
fn renders_change_column_null_with_backfill() {
    let sql = migration("    change_column_null :users, :age, false, 0");
    assert!(sql.contains("UPDATE \"users\" SET \"age\"=0 WHERE \"age\" IS NULL;"));
    assert!(sql.contains("ALTER TABLE \"users\" ALTER COLUMN \"age\" SET NOT NULL;"));
}

#[test]
fn follows_helper_methods_without_recursing_forever() {
    let sql = render(
        "class Example < ActiveRecord::Migration[7.1]\n  def change\n    add_things\n  end\n\n  def add_things\n    add_column :users, :age, :integer\n    add_things\n  end\nend\n",
    );
    assert_eq!(sql.matches("ADD \"age\"").count(), 1);
}

#[test]
fn renders_every_schema_statement() {
    let sql = migration(concat!(
        "    remove_columns :users, :a, :b\n",
        "    rename_column :users, :a, :b\n",
        "    rename_table :users, :people\n",
        "    remove_index :users, :email, algorithm: :concurrently, if_exists: true\n",
        "    rename_index :users, :old_name, :new_name\n",
        "    add_reference :posts, :author, foreign_key: true\n",
        "    remove_reference :posts, :author, polymorphic: true\n",
        "    add_foreign_key :posts, :users, validate: false\n",
        "    validate_foreign_key :posts, :users\n",
        "    remove_foreign_key :posts, :users\n",
        "    change_column :users, :age, :bigint, default: 1, null: false, using: 'age::bigint'\n",
        "    change_column_default :users, :age, from: 1, to: nil\n",
        "    change_column_default :users, :age, 2\n",
        "    add_check_constraint :users, 'age > 0', name: 'age_positive', validate: false\n",
        "    validate_check_constraint :users, name: 'age_positive'\n",
        "    remove_check_constraint :users, name: 'age_positive'\n",
        "    add_timestamps :users, null: true\n",
        "    remove_timestamps :users\n",
        "    drop_table :legacy, if_exists: true, force: :cascade\n",
        "    enable_extension :citext\n",
        "    disable_extension :hstore\n",
        "    create_enum :mood, ['happy', 'sad']\n",
        "    drop_enum :mood, if_exists: true\n",
        "    add_index :users, :email, if_not_exists: true\n",
        "    add_column :users, :nick, :string, if_not_exists: true",
    ));
    for expected in [
        "ALTER TABLE \"users\" DROP COLUMN \"a\", DROP COLUMN \"b\";",
        "ALTER TABLE \"users\" RENAME COLUMN \"a\" TO \"b\";",
        "ALTER TABLE \"users\" RENAME TO \"people\";",
        "DROP INDEX CONCURRENTLY IF EXISTS \"index_users_on_email\";",
        "\"author_id\" bigint",
        "VALIDATE CONSTRAINT",
        "NOT VALID",
        "ALTER COLUMN \"age\" DROP DEFAULT",
        "CONSTRAINT age_positive CHECK (age > 0) NOT VALID",
        "\"created_at\" timestamp(6)",
        "DROP TABLE IF EXISTS \"legacy\" CASCADE;",
        "CREATE EXTENSION IF NOT EXISTS \"citext\";",
        "DROP EXTENSION IF EXISTS \"hstore\" CASCADE;",
        "DROP TYPE IF EXISTS \"mood\";",
        "CREATE INDEX IF NOT EXISTS \"index_users_on_email\"",
        "ADD COLUMN IF NOT EXISTS \"nick\"",
    ] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
}

#[test]
fn renders_change_table_operations() {
    let sql = migration(concat!(
        "    change_table :users do |t|\n",
        "      t.string :nick\n",
        "      t.remove :legacy\n",
        "      t.rename :nick, :handle\n",
        "      t.change :age, :bigint\n",
        "      t.index :handle\n",
        "      t.remove_index :handle\n",
        "      t.references :team\n",
        "      t.check_constraint 'age > 0'\n",
        "      t.timestamps\n",
        "      t.remove_timestamps\n",
        "    end",
    ));
    for expected in [
        "ALTER TABLE \"users\" ADD \"nick\" character varying;",
        "ALTER TABLE \"users\" DROP COLUMN \"legacy\";",
        "RENAME COLUMN \"nick\" TO \"handle\"",
        "ALTER COLUMN \"age\" TYPE bigint",
        "CREATE INDEX \"index_users_on_handle\"",
        "DROP INDEX \"index_users_on_handle\"",
        "\"team_id\" bigint",
        "CHECK (age > 0)",
        "DROP COLUMN \"created_at\", DROP COLUMN \"updated_at\"",
    ] {
        assert!(sql.contains(expected), "falta {expected} en:\n{sql}");
    }
}
