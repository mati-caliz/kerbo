use crate::i18n::catalog::RuleText;
use crate::rules::{ComposeRule, MigrationRule};

const UNKNOWN_MIGRATION: RuleText = RuleText::new(
    "Migration rule",
    "follow the rule's advice",
    "squawk reports it by default.",
    "See the squawk documentation.",
);
const MIGRATION_TEXTS: [RuleText; MigrationRule::ALL.len()] = [
    RuleText::new(
        "Creates an index without CONCURRENTLY: writes to the table block while it is built",
        "create the index CONCURRENTLY, in a migration that does not run inside a transaction",
        "A plain CREATE INDEX takes a SHARE lock: reads go on, but every INSERT, UPDATE and DELETE waits until it finishes, which on a large table means minutes.",
        "CREATE INDEX idx_orders_customer ON orders (customer_id) becomes CREATE INDEX CONCURRENTLY idx_orders_customer ON orders (customer_id); in Alembic, postgresql_concurrently=True inside autocommit_block().",
    ),
    RuleText::new(
        "Drops an index without CONCURRENTLY: reads and writes to the table block",
        "drop the index with DROP INDEX CONCURRENTLY",
        "DROP INDEX takes an ACCESS EXCLUSIVE lock on the table: nobody reads or writes it until it finishes, and every incoming query queues behind it.",
        "DROP INDEX idx_old; becomes DROP INDEX CONCURRENTLY idx_old;",
    ),
    RuleText::new(
        "Adds a NOT NULL column without a default to an existing table",
        "add it nullable or with a non-volatile DEFAULT, and backfill before requiring it",
        "Existing rows have no value: the migration fails if the table has data, and deployed code that inserts without the column starts failing.",
        "ADD COLUMN country text NOT NULL becomes ADD COLUMN country text NOT NULL DEFAULT 'AR', or a nullable column plus a backfill.",
    ),
    RuleText::new(
        "Sets an existing column NOT NULL: scans the whole table with reads and writes blocked",
        "add a CHECK (column IS NOT NULL) NOT VALID, validate it separately and only then SET NOT NULL",
        "SET NOT NULL takes ACCESS EXCLUSIVE and scans every row to verify it: on a large table, everything that uses it waits.",
        "ADD CONSTRAINT t_col_not_null CHECK (col IS NOT NULL) NOT VALID; VALIDATE CONSTRAINT t_col_not_null; and then ALTER COLUMN col SET NOT NULL.",
    ),
    RuleText::new(
        "Changes a column type: rewrites the table with reads and writes blocked",
        "add a new column with the new type, copy the data in batches and switch the code before dropping the old one",
        "ALTER COLUMN ... TYPE takes ACCESS EXCLUSIVE and almost always rewrites every row; it also breaks clients that expect the old type.",
        "Instead of ALTER COLUMN amount TYPE numeric, add amount_decimal numeric, backfill it and move the reads to the new column.",
    ),
    RuleText::new(
        "Renames a column the deployed code may still use",
        "add the new column, write to both, move the reads and drop the old one in a later deploy",
        "During a deploy the old and new code run side by side: the old one keeps asking for the previous name and fails as soon as the migration runs.",
        "RENAME COLUMN name TO full_name is replaced by ADD COLUMN full_name with a backfill; dropping name goes in a later migration.",
    ),
    RuleText::new(
        "Renames a table the deployed code may still use",
        "create the new table (or a view with the old name) and drop the old one once no deploy uses it",
        "Code that is already running keeps querying the old name: it fails from the moment the migration runs until the deploy finishes.",
        "RENAME TO customers is replaced by the customers table plus a users view that reads it, and the view is dropped in another migration.",
    ),
    RuleText::new(
        "Drops a column the deployed code may still read",
        "stop using it in the code, deploy, and only then drop it",
        "Clients that still read or write it fail as soon as it disappears, and the drop cannot be undone without a backup.",
        "First the code stops reading discount; in the next deploy, ALTER TABLE orders DROP COLUMN discount.",
    ),
    RuleText::new(
        "Drops a table the deployed code may still use",
        "stop using it in the code, deploy, and only then drop it",
        "The old code keeps querying it during the deploy, and the data does not come back without a backup.",
        "First every use of old_audit goes away; in a later migration, DROP TABLE old_audit.",
    ),
    RuleText::new(
        "An operation that takes locks without a lock_timeout first",
        "add SET lock_timeout = '1s'; at the start of the migration",
        "If the table is busy, the migration waits for the lock and every query behind it queues up: one stuck ALTER takes the application down.",
        "ALTER TABLE orders ADD COLUMN note text; becomes SET lock_timeout = '1s'; ALTER TABLE orders ADD COLUMN note text;",
    ),
    RuleText::new(
        "A slow operation without a statement_timeout first",
        "add SET statement_timeout = '5s'; at the start of the migration",
        "Without a cap, an operation that scans a large table can hold its lock for minutes.",
        "SET statement_timeout = '5s'; before the first ALTER, CREATE or DROP.",
    ),
    RuleText::new(
        "Drops a constraint the code may take for granted",
        "stop relying on the constraint in the code before dropping it, or replace it with an equivalent one",
        "The deployed code assumes the foreign key, UNIQUE or CHECK is still there; without it, data that used to be impossible gets in.",
        "DROP CONSTRAINT orders_customer_fk goes in a migration after the one that stops relying on it.",
    ),
    RuleText::new(
        "timestamp column without a time zone",
        "use timestamptz",
        "Postgres stores the date and drops the offset: 21:00-03 and 21:00+02 end up equal even though they are different instants.",
        "created_at timestamp becomes created_at timestamptz.",
    ),
    RuleText::new(
        "32-bit integer column",
        "use bigint (or bigserial)",
        "An int tops out at 2,147,483,647: a growing id or counter fills it up, and changing the type later rewrites the table.",
        "id serial becomes id bigint GENERATED BY DEFAULT AS IDENTITY.",
    ),
    RuleText::new(
        "16-bit integer column",
        "use bigint",
        "A smallint tops out at 32,767: it fills up fast, and widening it later rewrites the table.",
        "quantity smallint becomes quantity bigint.",
    ),
    RuleText::new(
        "serial column instead of an identity",
        "use GENERATED BY DEFAULT AS IDENTITY",
        "serial creates a separate sequence with its own owner and permissions; an identity column is part of the table and is copied and migrated with it.",
        "id serial becomes id integer GENERATED BY DEFAULT AS IDENTITY.",
    ),
    RuleText::new(
        "varchar column with a maximum length",
        "use text with a CHECK on the length",
        "Changing a varchar's length takes ACCESS EXCLUSIVE; a CHECK can be replaced without locking the table.",
        "name varchar(255) becomes name text CHECK (char_length(name) <= 255).",
    ),
    RuleText::new(
        "Adds a constraint that validates the whole table with writes blocked",
        "add it NOT VALID and validate it in another transaction",
        "Without NOT VALID, Postgres scans every row while blocking writes; validating separately only needs a lock that still allows writes.",
        "ADD CONSTRAINT positive_amount CHECK (amount > 0) NOT VALID; and in another migration VALIDATE CONSTRAINT positive_amount.",
    ),
    RuleText::new(
        "Adds a foreign key that blocks writes on both tables",
        "add it NOT VALID and validate it in another transaction",
        "The foreign key takes SHARE ROW EXCLUSIVE on both tables and scans them: nobody writes to either until it finishes.",
        "ADD CONSTRAINT orders_customer_fk FOREIGN KEY (customer_id) REFERENCES customers (id) NOT VALID; then VALIDATE CONSTRAINT.",
    ),
    RuleText::new(
        "Adds a primary key that builds its index with the table locked",
        "build the unique index CONCURRENTLY and add the key USING INDEX",
        "ADD PRIMARY KEY builds the index under ACCESS EXCLUSIVE: no reads or writes while it runs.",
        "CREATE UNIQUE INDEX CONCURRENTLY orders_pk ON orders (id); ADD CONSTRAINT orders_pkey PRIMARY KEY USING INDEX orders_pk.",
    ),
    RuleText::new(
        "Adds a UNIQUE that builds its index with the table locked",
        "build the unique index CONCURRENTLY and add the constraint USING INDEX",
        "The UNIQUE builds its index under ACCESS EXCLUSIVE: the table has no reads or writes while it runs.",
        "CREATE UNIQUE INDEX CONCURRENTLY users_email ON users (email); ADD CONSTRAINT users_email_unique UNIQUE USING INDEX users_email.",
    ),
    RuleText::new(
        "Adds a column with a volatile default or a generated column: rewrites the table",
        "add it nullable, backfill it in batches and set the default afterwards",
        "Since Postgres 11 a constant default does not rewrite; a volatile one (random(), gen_random_uuid()) or a generated column does, under ACCESS EXCLUSIVE.",
        "ADD COLUMN token uuid DEFAULT gen_random_uuid() becomes ADD COLUMN token uuid, a backfill and SET DEFAULT gen_random_uuid().",
    ),
    RuleText::new(
        "Adds an enum value without BEFORE or AFTER",
        "state the position with BEFORE or AFTER",
        "Without a position the value goes last, and the enum order drives ORDER BY and comparisons.",
        "ADD VALUE 'paused' becomes ADD VALUE 'paused' AFTER 'active'.",
    ),
    RuleText::new(
        "Drops a type that the code or columns may use",
        "stop using it before dropping it",
        "Columns, functions or clients that use it fail as soon as it is gone.",
        "DROP TYPE old_status goes in a migration after the one that stops using it.",
    ),
    RuleText::new(
        "Drops a function the code may call",
        "stop calling it before dropping it",
        "Deployed code that calls it fails during the deploy.",
        "DROP FUNCTION compute_total goes after every call is removed.",
    ),
    RuleText::new(
        "Drops a sequence that the code or a default may use",
        "stop using it before dropping it",
        "A default with nextval() or a client that reads it fails as soon as it is gone.",
        "DROP SEQUENCE orders_seq goes after every use is removed.",
    ),
    RuleText::new(
        "Drops a view the code may query",
        "stop querying it before dropping it",
        "Deployed code that reads it fails during the deploy.",
        "DROP VIEW monthly_report goes after every query is removed.",
    ),
    RuleText::new(
        "Removes a column's default",
        "make sure all the code sends the value before removing it",
        "INSERTs that leave the column out stop getting the default: they store NULL or fail.",
        "DROP DEFAULT goes after every INSERT sends the column.",
    ),
    RuleText::new(
        "Removes a column's NOT NULL",
        "make sure the code tolerates NULL before removing it",
        "Code that assumes there is always a value starts getting NULL.",
        "DROP NOT NULL goes after all the code handles NULL.",
    ),
    RuleText::new(
        "Renames an object the deployed code may use",
        "create the new one, move the code over and drop the old one in another deploy",
        "During the deploy the old code keeps using the previous name and fails.",
        "RENAME CONSTRAINT and RENAME TO on indexes, types, views or functions are replaced by creating the new one and dropping the old one later.",
    ),
    RuleText::new(
        "A name longer than 63 bytes",
        "shorten it",
        "Postgres silently truncates it to 63 bytes: two names that differ after byte 63 collide, and code using the full name cannot find it.",
        "An index with a 70-character name becomes one with 63 or fewer.",
    ),
    RuleText::new(
        "BEGIN, COMMIT or ROLLBACK in a migration that already runs in a transaction",
        "remove them, or split the migration into files",
        "The migration tool already opens the transaction: an extra BEGIN does not nest, and a COMMIT ends the transaction halfway.",
        "A BEGIN; ... COMMIT; inside a Prisma or drizzle migration is removed.",
    ),
    RuleText::new(
        "CREATE INDEX CONCURRENTLY inside a transaction",
        "move it to a migration that does not run in a transaction",
        "Postgres does not allow CONCURRENTLY inside a transaction: the migration fails.",
        "Move the CREATE INDEX CONCURRENTLY to its own file, without a transaction.",
    ),
    RuleText::new(
        "A statement that fails if the migration runs twice",
        "use IF EXISTS or IF NOT EXISTS",
        "Outside a transaction, if the migration stops halfway what already ran stays done, and retrying fails on the first one.",
        "CREATE INDEX CONCURRENTLY idx ON t (c) becomes CREATE INDEX CONCURRENTLY IF NOT EXISTS idx ON t (c).",
    ),
    RuleText::new(
        "Drops an entire database",
        "do not drop databases from a migration",
        "All the data is lost and everything that uses it stops working; there is no way back without a backup.",
        "DROP DATABASE old is done by hand, outside migrations, after a backup.",
    ),
    RuleText::new(
        "Fixed-length char column",
        "use text (or varchar without a length)",
        "char pads with spaces up to the length: comparisons and lengths surprise, and it saves no space.",
        "code char(3) becomes code text.",
    ),
    RuleText::new(
        "TRUNCATE ... CASCADE also empties the tables that reference it",
        "truncate each table on purpose, without CASCADE",
        "CASCADE follows foreign keys and empties tables the migration never names.",
        "TRUNCATE orders CASCADE becomes TRUNCATE orders, order_items with the tables spelled out.",
    ),
    RuleText::new(
        "Drops an entire schema",
        "stop using it before dropping it",
        "Everything in the schema disappears and the code using it fails.",
        "DROP SCHEMA old goes after moving or deliberately dropping what it holds.",
    ),
    RuleText::new(
        "Drops a domain that columns may use",
        "stop using it before dropping it",
        "Columns and functions with that type fail as soon as it is gone.",
        "DROP DOMAIN email_t goes after changing the columns that use it.",
    ),
    RuleText::new(
        "Drops an extension",
        "stop using its types and functions before dropping it",
        "Columns, indexes and functions that depend on the extension fail or get dropped with it.",
        "DROP EXTENSION pg_trgm goes after removing the indexes that use it.",
    ),
    RuleText::new(
        "Moves an object to another schema",
        "create the object in the new schema and move the code before removing the old one",
        "The deployed code looks it up under the previous schema and fails.",
        "ALTER TABLE t SET SCHEMA new is replaced by a migration across two deploys.",
    ),
    RuleText::new(
        "Removes a generated column's expression",
        "move the code to write the column before removing it",
        "The column stops computing itself: code that only read it starts seeing stale values or NULL.",
        "DROP EXPRESSION goes after the code writes the column.",
    ),
    RuleText::new(
        "Changes a column's identity",
        "leave the identity as it is, or make the change with the code already adapted",
        "INSERTs that send the value fail with GENERATED ALWAYS, and those that do not send it fail without an identity.",
        "ALTER COLUMN id SET GENERATED ALWAYS goes after no INSERT sends the id.",
    ),
    RuleText::new(
        "Creates a domain with constraints",
        "put the constraints on the columns, not on the domain",
        "Changing a domain constraint later revalidates every column of every table that uses it.",
        "CREATE DOMAIN amount AS numeric CHECK (VALUE > 0) becomes CREATE DOMAIN amount AS numeric plus a CHECK on the column.",
    ),
    RuleText::new(
        "Adds a constraint to a domain",
        "add the constraint on the columns with NOT VALID",
        "ADD CONSTRAINT on a domain scans every column that uses it with the tables locked.",
        "ALTER DOMAIN amount ADD CHECK (VALUE > 0) is replaced by a CHECK ... NOT VALID on each column.",
    ),
    RuleText::new(
        "REINDEX without CONCURRENTLY",
        "use REINDEX ... CONCURRENTLY",
        "A regular REINDEX blocks the table's writes while it rebuilds the index.",
        "REINDEX INDEX idx becomes REINDEX INDEX CONCURRENTLY idx.",
    ),
    RuleText::new(
        "DETACH PARTITION without CONCURRENTLY",
        "use DETACH PARTITION ... CONCURRENTLY",
        "Without CONCURRENTLY it takes ACCESS EXCLUSIVE on the whole partitioned table.",
        "DETACH PARTITION p2024 becomes DETACH PARTITION p2024 CONCURRENTLY.",
    ),
    RuleText::new(
        "A BEGIN that is never closed",
        "add the COMMIT at the end",
        "The transaction stays open with its locks until the connection drops.",
        "BEGIN; ... becomes BEGIN; ... COMMIT;",
    ),
    RuleText::new(
        "The same column appears twice",
        "keep just one",
        "Postgres rejects the statement or keeps one value without saying which.",
        "INSERT INTO t (a, b, a) becomes INSERT INTO t (a, b).",
    ),
    RuleText::new(
        "A squawk-ignore names a rule that does not exist",
        "fix the rule name",
        "The comment silences nothing: the report it meant to ignore keeps showing up and the misspelled name stays there.",
        "-- squawk-ignore ban-drop-colum becomes -- squawk-ignore ban-drop-column.",
    ),
];

pub fn migration(rule: MigrationRule) -> RuleText {
    MIGRATION_TEXTS
        .get(rule.position())
        .copied()
        .unwrap_or(UNKNOWN_MIGRATION)
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
        ComposeRule::CpuLimit => RuleText::new(
            "The service has no CPU limit",
            "set deploy.resources.limits.cpus (or cpus) on every service",
            "Without a limit, a service stuck in a loop or hit by a spike takes every core on the host and the other containers stop responding.",
            "deploy: { resources: { limits: { cpus: \"1.0\" } } }",
        ),
        ComposeRule::Restart => RuleText::new(
            "The service does not say what to do when it goes down",
            "set restart (unless-stopped, on-failure, or \"no\" for a one-off task)",
            "Without a restart policy, a service that crashes on an error or an OOM stays down until someone brings it up by hand.",
            "restart: unless-stopped",
        ),
        ComposeRule::ImageTag => RuleText::new(
            "The image has no pinned version",
            "pin a version tag, or a digest",
            "Without a tag (or with latest) every pull may bring a different version: the same compose file starts different things on each machine and an update breaks things nobody asked for.",
            "image: postgres becomes image: postgres:16.4.",
        ),
        ComposeRule::DependsOnHealthy => RuleText::new(
            "Depends on a service with a healthcheck without waiting for it to be healthy",
            "use depends_on with condition: service_healthy",
            "Without the condition, compose starts the service as soon as the other one started, not when it is ready: the application tries to connect to a database that does not accept connections yet.",
            "depends_on: [db] becomes depends_on: { db: { condition: service_healthy } }.",
        ),
    }
}
