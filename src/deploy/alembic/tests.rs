use tree_sitter::Parser;

use super::violations;
use crate::language::Language;
use crate::rules::{MigrationRule, RuleId};

const LOCKING_RULES: [MigrationRule; 9] = [
    MigrationRule::RequireConcurrentIndexCreation,
    MigrationRule::RequireConcurrentIndexDeletion,
    MigrationRule::AddingRequiredField,
    MigrationRule::AddingNotNullableField,
    MigrationRule::ChangingColumnType,
    MigrationRule::RenamingColumn,
    MigrationRule::RenamingTable,
    MigrationRule::BanDropColumn,
    MigrationRule::BanDropTable,
];

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
        .filter(|(rule, _, _)| LOCKING_RULES.contains(rule))
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
            (MigrationRule::ChangingColumnType, 15, "email".to_owned()),
            (MigrationRule::AddingNotNullableField, 15, "email".to_owned()),
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

fn all_found(source: &str) -> Vec<(MigrationRule, usize)> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    violations(tree.root_node(), source.as_bytes())
        .into_iter()
        .filter_map(|violation| {
            if let RuleId::Migration(rule) = violation.rule {
                Some((rule, violation.line))
            } else {
                None
            }
        })
        .collect()
}

const LOOPS_HELPERS_AND_AUTOCOMMIT: &str = r#"from alembic import op
import sqlalchemy as sa

revision = "b"
down_revision = "a"


def _drop(table, column):
    op.drop_column(table, column)


def upgrade():
    for table in ["users", "orders"]:
        op.drop_column(table, "legacy")
    _drop("events", "old")
    with op.get_context().autocommit_block():
        op.create_index("ix_users_email", "users", ["email"], postgresql_concurrently=True)
    op.create_index("ix_users_name", "users", ["name"], postgresql_concurrently=True)
"#;

#[test]
fn unrolls_loops_follows_helpers_and_leaves_the_transaction_in_autocommit_blocks() {
    let found = all_found(LOOPS_HELPERS_AND_AUTOCOMMIT);
    let drops: Vec<usize> = found
        .iter()
        .filter(|(rule, _)| *rule == MigrationRule::BanDropColumn)
        .map(|(_, line)| *line)
        .collect();
    assert_eq!(drops, [14, 14, 9]);
    let concurrent_in_transaction: Vec<usize> = found
        .iter()
        .filter(|(rule, _)| *rule == MigrationRule::BanConcurrentIndexCreationInTransaction)
        .map(|(_, line)| *line)
        .collect();
    assert_eq!(concurrent_in_transaction, [18]);
    assert!(found.contains(&(MigrationRule::RequireLockTimeout, 14)));
}

#[test]
fn renders_column_types_defaults_and_constraints_like_alembic() {
    let source = r#"from alembic import op
import sqlalchemy as sa

revision = "a"
down_revision = None


def upgrade():
    op.add_column("users", sa.Column("visits", sa.Integer(), server_default=sa.text("random()")))
    op.add_column("users", sa.Column("seen", sa.DateTime(), server_default=sa.func.now()))
    op.add_column("users", sa.Column("code", sa.CHAR(3)))
    op.add_column("users", sa.Column("team_id", sa.BigInteger(), sa.ForeignKey("teams.id"), index=True))
    op.create_unique_constraint("uq_users_code", "users", ["code"])
    op.drop_constraint("fk_old", "users", type_="foreignkey")
"#;
    let found = all_found(source);
    for expected in [
        (MigrationRule::PreferBigintOverInt, 9),
        (MigrationRule::AddingFieldWithDefault, 9),
        (MigrationRule::PreferTimestampTz, 10),
        (MigrationRule::BanCharField, 11),
        (MigrationRule::AddingForeignKeyConstraint, 12),
        (MigrationRule::RequireConcurrentIndexCreation, 12),
        (MigrationRule::DisallowedUniqueConstraint, 13),
        (MigrationRule::BanDropConstraint, 14),
    ] {
        assert!(found.contains(&expected), "falta {expected:?} en {found:?}");
    }
    assert!(!found.contains(&(MigrationRule::AddingFieldWithDefault, 10)));
}
