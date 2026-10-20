use crate::i18n::catalog::RuleText;
use crate::rules::{ComposeRule, MigrationRule};

pub const fn migration(rule: MigrationRule) -> RuleText {
    match rule {
        MigrationRule::RequireConcurrentIndexCreation => RuleText::new(
            "Creates an index without CONCURRENTLY: writes to the table block while it is built",
            "create the index CONCURRENTLY, in a migration that does not run inside a transaction",
            "A plain CREATE INDEX takes a SHARE lock: reads go on, but every INSERT, UPDATE and DELETE waits until it finishes, which on a large table means minutes.",
            "CREATE INDEX idx_orders_customer ON orders (customer_id) becomes CREATE INDEX CONCURRENTLY idx_orders_customer ON orders (customer_id); in Alembic, postgresql_concurrently=True inside autocommit_block().",
        ),
        MigrationRule::RequireConcurrentIndexDeletion => RuleText::new(
            "Drops an index without CONCURRENTLY: reads and writes to the table block",
            "drop the index with DROP INDEX CONCURRENTLY",
            "DROP INDEX takes an ACCESS EXCLUSIVE lock on the table: nobody reads or writes it until it finishes, and every incoming query queues behind it.",
            "DROP INDEX idx_old; becomes DROP INDEX CONCURRENTLY idx_old;",
        ),
        MigrationRule::AddingRequiredField => RuleText::new(
            "Adds a NOT NULL column without a default to an existing table",
            "add it nullable or with a non-volatile DEFAULT, and backfill before requiring it",
            "Existing rows have no value: the migration fails if the table has data, and deployed code that inserts without the column starts failing.",
            "ADD COLUMN country text NOT NULL becomes ADD COLUMN country text NOT NULL DEFAULT 'AR', or a nullable column plus a backfill.",
        ),
        MigrationRule::AddingNotNullableField => RuleText::new(
            "Sets an existing column NOT NULL: scans the whole table with reads and writes blocked",
            "add a CHECK (column IS NOT NULL) NOT VALID, validate it separately and only then SET NOT NULL",
            "SET NOT NULL takes ACCESS EXCLUSIVE and scans every row to verify it: on a large table, everything that uses it waits.",
            "ADD CONSTRAINT t_col_not_null CHECK (col IS NOT NULL) NOT VALID; VALIDATE CONSTRAINT t_col_not_null; and then ALTER COLUMN col SET NOT NULL.",
        ),
        MigrationRule::ChangingColumnType => RuleText::new(
            "Changes a column type: rewrites the table with reads and writes blocked",
            "add a new column with the new type, copy the data in batches and switch the code before dropping the old one",
            "ALTER COLUMN ... TYPE takes ACCESS EXCLUSIVE and almost always rewrites every row; it also breaks clients that expect the old type.",
            "Instead of ALTER COLUMN amount TYPE numeric, add amount_decimal numeric, backfill it and move the reads to the new column.",
        ),
        MigrationRule::RenamingColumn => RuleText::new(
            "Renames a column the deployed code may still use",
            "add the new column, write to both, move the reads and drop the old one in a later deploy",
            "During a deploy the old and new code run side by side: the old one keeps asking for the previous name and fails as soon as the migration runs.",
            "RENAME COLUMN name TO full_name is replaced by ADD COLUMN full_name with a backfill; dropping name goes in a later migration.",
        ),
        MigrationRule::RenamingTable => RuleText::new(
            "Renames a table the deployed code may still use",
            "create the new table (or a view with the old name) and drop the old one once no deploy uses it",
            "Code that is already running keeps querying the old name: it fails from the moment the migration runs until the deploy finishes.",
            "RENAME TO customers is replaced by the customers table plus a users view that reads it, and the view is dropped in another migration.",
        ),
        MigrationRule::BanDropColumn => RuleText::new(
            "Drops a column the deployed code may still read",
            "stop using it in the code, deploy, and only then drop it",
            "Clients that still read or write it fail as soon as it disappears, and the drop cannot be undone without a backup.",
            "First the code stops reading discount; in the next deploy, ALTER TABLE orders DROP COLUMN discount.",
        ),
        MigrationRule::BanDropTable => RuleText::new(
            "Drops a table the deployed code may still use",
            "stop using it in the code, deploy, and only then drop it",
            "The old code keeps querying it during the deploy, and the data does not come back without a backup.",
            "First every use of old_audit goes away; in a later migration, DROP TABLE old_audit.",
        ),
    }
}

pub const fn compose(rule: ComposeRule) -> RuleText {
    match rule {
        ComposeRule::MemoryLimit => RuleText::new(
            "The service has no memory limit",
            "set deploy.resources.limits.memory (or mem_limit) on every service",
            "Without a limit, a service with a memory leak eats the host's memory and the OOM killer kills any other one, sometimes the database.",
            "deploy: { resources: { limits: { memory: 512M } } }",
        ),
        ComposeRule::HealthcheckStartPeriod => RuleText::new(
            "The healthcheck has no start_period",
            "add a start_period as long as the service takes to start",
            "Without start_period, checks that fail during startup count as retries: a slow service ends up unhealthy and the ones that depend on it never start.",
            "healthcheck: { test: [\"CMD\", \"pg_isready\"], interval: 10s, timeout: 5s, retries: 5, start_period: 30s }",
        ),
        ComposeRule::HealthcheckInterval => RuleText::new(
            "The healthcheck interval is unreasonable: between 5 s and 5 min, with a timeout shorter than the interval",
            "use an interval between 5s and 5m and a timeout shorter than the interval",
            "A very short interval loads the service and marks it down on a spike; a very long one takes minutes to notice an outage; a timeout equal or longer overlaps one check with the next.",
            "interval: 1s becomes interval: 10s with timeout: 5s.",
        ),
    }
}
