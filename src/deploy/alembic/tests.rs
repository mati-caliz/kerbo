use tree_sitter::Parser;

use super::violations;
use crate::language::Language;
use crate::rules::{MigrationRule, RuleId};

fn found(source: &str) -> Vec<(MigrationRule, usize, String)> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    violations(tree.root_node(), source.as_bytes())
        .into_iter()
        .map(|violation| {
            let RuleId::Migration(rule) = violation.rule else {
                panic!("regla inesperada {:?}", violation.rule);
            };
            (rule, violation.line, violation.subject)
        })
        .collect()
}

const MIGRATION: &str = r#"from alembic import context, op
import sqlalchemy as sa

revision = "abc"


def upgrade():
    op.create_table("tags", sa.Column("id", sa.Integer(), primary_key=True))
    op.create_index("ix_tags_id", "tags", ["id"])
    op.create_index("ix_users_email", "users", ["email"])
    op.create_index("ix_users_name", "users", ["name"], postgresql_concurrently=True)
    op.add_column("users", sa.Column("plan", sa.String(), nullable=False))
    op.add_column("users", sa.Column("country", sa.String(), nullable=False, server_default="AR"))
    op.add_column("users", sa.Column("total", sa.Integer(), sa.Computed("a + b"), nullable=False, index=True))
    op.alter_column("users", "email", nullable=False, type_=sa.Text(), new_column_name="mail")
    op.drop_column("users", "legacy")
    op.drop_table("old")
    op.rename_table("people", "persons")
    op.drop_index("ix_old", table_name="users")
    with op.batch_alter_table("orders") as batch_op:
        batch_op.drop_column("note")
        batch_op.create_index("ix_orders_total", ["total"])
    op.execute("ALTER TABLE orders DROP COLUMN discount")
    op.execute(sa.text("""
        CREATE INDEX ix_x ON orders (x)
    """))
    if op.get_bind().dialect.name == "mysql":
        op.drop_table("mysql_only")
    elif op.get_bind().dialect.name == "postgresql":
        op.drop_table("postgres_only")
    else:
        op.drop_table("other_dialects")
    if op.get_bind().dialect.name != "sqlite":
        op.drop_table("not_sqlite")
    conn = op.get_bind()
    conn.execute(sa.text('ALTER TABLE "tag" ALTER COLUMN "type" TYPE VARCHAR'))


def downgrade():
    op.drop_table("users")
"#;

#[test]
fn maps_alembic_operations_to_the_squawk_rules() {
    assert_eq!(
        found(MIGRATION),
        vec![
            (
                MigrationRule::RequireConcurrentIndexCreation,
                10,
                "users".to_owned()
            ),
            (MigrationRule::AddingRequiredField, 12, "plan".to_owned()),
            (
                MigrationRule::RequireConcurrentIndexCreation,
                14,
                "users".to_owned()
            ),
            (MigrationRule::AddingNotNullableField, 15, "email".to_owned()),
            (MigrationRule::ChangingColumnType, 15, "email".to_owned()),
            (MigrationRule::RenamingColumn, 15, "email".to_owned()),
            (MigrationRule::BanDropColumn, 16, "legacy".to_owned()),
            (MigrationRule::BanDropTable, 17, "old".to_owned()),
            (MigrationRule::RenamingTable, 18, "people".to_owned()),
            (
                MigrationRule::RequireConcurrentIndexDeletion,
                19,
                "ix_old".to_owned()
            ),
            (MigrationRule::BanDropColumn, 21, "note".to_owned()),
            (
                MigrationRule::RequireConcurrentIndexCreation,
                22,
                "orders".to_owned()
            ),
            (MigrationRule::BanDropColumn, 23, "discount".to_owned()),
            (
                MigrationRule::RequireConcurrentIndexCreation,
                25,
                "orders".to_owned()
            ),
            (MigrationRule::BanDropTable, 30, "postgres_only".to_owned()),
            (MigrationRule::BanDropTable, 34, "not_sqlite".to_owned()),
            (MigrationRule::ChangingColumnType, 36, "type".to_owned()),
        ]
    );
}

#[test]
fn only_reads_alembic_upgrades() {
    assert_eq!(found("def upgrade():\n    op.drop_table('x')\n"), Vec::new());
    assert_eq!(
        found("import alembic\n\ndef downgrade():\n    op.drop_table('x')\n"),
        Vec::new()
    );
}
