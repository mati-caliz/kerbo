use super::rendering::assert_rendered;
use super::{MemoryLoader, sql};

const APP: &str = "casos";
const INITIAL: &str =
    include_str!("../../../../scripts/parity/django/casos/casos/migrations/0001_initial.py");
const BIG_IDS: &str =
    include_str!("../../../../scripts/parity/django/casos/casos/migrations/0002_big_author_ids.py");
const UNIQUE_TOGETHER: &str =
    include_str!("../../../../scripts/parity/django/casos/casos/migrations/0003_drop_unique_together.py");
const RETARGET: &str =
    include_str!("../../../../scripts/parity/django/casos/casos/migrations/0004_retarget_tags.py");
const COMMENTS: &str =
    include_str!("../../../../scripts/parity/django/casos/casos/migrations/0005_comments_and_collations.py");
const POSTGRES: &str =
    include_str!("../../../../scripts/parity/django/casos/casos/migrations/0006_postgres_operations.py");
const POSITION: &str =
    include_str!("../../../../scripts/parity/django/casos/casos/migrations/0007_tag_slug.py");
const RENAME_INDEX: &str =
    include_str!("../../../../scripts/parity/django/casos/casos/migrations/0008_rename_slug_index.py");

fn history(until: usize) -> MemoryLoader {
    [
        ("0001_initial", INITIAL),
        ("0002_big_author_ids", BIG_IDS),
        ("0003_drop_unique_together", UNIQUE_TOGETHER),
        ("0004_retarget_tags", RETARGET),
        ("0005_comments_and_collations", COMMENTS),
        ("0006_postgres_operations", POSTGRES),
        ("0007_tag_slug", POSITION),
    ]
    .into_iter()
    .take(until)
    .fold(MemoryLoader::default(), |loader, (stem, source)| {
        loader.with(APP, stem, source)
    })
}

#[test]
fn renders_comments_and_collations_on_create_and_alter() {
    assert_rendered(
        &sql(&history(0), APP, "0001_initial", INITIAL),
        &[
            "\"code\" varchar(20) COLLATE \"C\" NOT NULL",
            "COMMENT ON TABLE \"casos_author\" IS 'Autores de libros'",
            "COMMENT ON COLUMN \"casos_author\".\"name\" IS 'Nombre visible'",
        ],
        &[],
    );
    assert_rendered(
        &sql(&history(4), APP, "0005_comments_and_collations", COMMENTS),
        &[
            "ALTER TABLE \"casos_author\" ALTER COLUMN \"name\" TYPE varchar(100);",
            "COMMENT ON COLUMN \"casos_author\".\"name\" IS 'Nombre público'",
            "ALTER TABLE \"casos_author\" ALTER COLUMN \"code\" TYPE varchar(20);",
            "ALTER COLUMN \"title\" TYPE varchar(200) COLLATE \"C\";",
            "COMMENT ON COLUMN \"casos_book\".\"summary\" IS 'Resumen'",
            "COMMENT ON TABLE \"casos_book\" IS 'Libros'",
        ],
        &[],
    );
}

#[test]
fn retypes_the_foreign_keys_pointing_to_a_widened_primary_key() {
    assert_rendered(
        &sql(&history(1), APP, "0002_big_author_ids", BIG_IDS),
        &[
            "SET CONSTRAINTS \"casos_book_author_id_af6490d8_fk_casos_author_id\" IMMEDIATE; ALTER TABLE \"casos_book\" DROP CONSTRAINT",
            "ALTER SEQUENCE IF EXISTS \"casos_author_id_seq\" AS bigint",
            "ALTER TABLE \"casos_book\" ALTER COLUMN \"author_id\" TYPE bigint USING \"author_id\"::bigint",
            "ADD CONSTRAINT \"casos_book_author_id_af6490d8_fk\" FOREIGN KEY (\"author_id\")",
            "ALTER TABLE \"casos_book_tags\" ALTER COLUMN \"tag_id\" TYPE bigint",
        ],
        &[],
    );
}

#[test]
fn drops_removed_unique_together_and_renames_unnamed_indexes() {
    assert_rendered(
        &sql(&history(2), APP, "0003_drop_unique_together", UNIQUE_TOGETHER),
        &[
            "ALTER TABLE \"casos_book\" DROP CONSTRAINT \"casos_book_title_author_id_f5f6c028_uniq\"",
            "ADD CONSTRAINT \"casos_book_isbn_author_id_95fcc130_uniq\" UNIQUE",
        ],
        &[],
    );
    assert_rendered(
        &sql(&history(7), APP, "0008_rename_slug_index", RENAME_INDEX),
        &["ALTER INDEX \"casos_tag_position_"],
        &[],
    );
}

#[test]
fn retargets_many_to_many_through_tables() {
    assert_rendered(
        &sql(&history(3), APP, "0004_retarget_tags", RETARGET),
        &[
            "ALTER TABLE \"casos_book_tags\" RENAME COLUMN \"tag_id\" TO \"label_id\"",
            "ALTER TABLE \"casos_book_tags\" ALTER COLUMN \"label_id\" TYPE integer USING \"label_id\"::integer",
            "ADD CONSTRAINT \"casos_book_tags_label_id_ffafc892_fk_casos_label_id\" FOREIGN KEY",
            "ALTER TABLE \"casos_book_tags\" RENAME TO \"casos_book_labels\"",
        ],
        &[],
    );
}

#[test]
fn renders_postgres_operations() {
    assert_rendered(
        &sql(&history(5), APP, "0006_postgres_operations", POSTGRES),
        &[
            "CREATE COLLATION \"case_insensitive\" (locale=\"und-u-ks-level2\", provider=\"icu\", deterministic=false)",
            "DROP COLLATION \"german\"",
            "CREATE INDEX CONCURRENTLY \"book_isbn_idx\"",
            "CHECK (\"isbn\"::text ~  E'^\\\\d+$') NOT VALID",
            "ALTER TABLE \"casos_book\" VALIDATE CONSTRAINT \"book_isbn_digits\"",
        ],
        &[],
    );
}
