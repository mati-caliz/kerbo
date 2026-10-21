use std::fmt;

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::id::RuleId;

const EXPECTED_RULE: &str = "una regla de kerbo";

macro_rules! rule_family {
    ($family:ident { $($variant:ident => $name:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $family {
            $($variant),+
        }

        impl $family {
            pub const ALL: [Self; [$($name),+].len()] = [$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }

            pub fn position(self) -> usize {
                Self::ALL.iter().position(|rule| *rule == self).unwrap_or_default()
            }
        }
    };
}

rule_family!(MigrationRule {
    RequireConcurrentIndexCreation => "require-concurrent-index-creation",
    RequireConcurrentIndexDeletion => "require-concurrent-index-deletion",
    AddingRequiredField => "adding-required-field",
    AddingNotNullableField => "adding-not-nullable-field",
    ChangingColumnType => "changing-column-type",
    RenamingColumn => "renaming-column",
    RenamingTable => "renaming-table",
    BanDropColumn => "ban-drop-column",
    BanDropTable => "ban-drop-table",
    RequireLockTimeout => "require-lock-timeout",
    RequireStatementTimeout => "require-statement-timeout",
    BanDropConstraint => "ban-drop-constraint",
    PreferTimestampTz => "prefer-timestamp-tz",
    PreferBigintOverInt => "prefer-bigint-over-int",
    PreferBigintOverSmallint => "prefer-bigint-over-smallint",
    PreferIdentity => "prefer-identity",
    PreferTextField => "prefer-text-field",
    ConstraintMissingNotValid => "constraint-missing-not-valid",
    AddingForeignKeyConstraint => "adding-foreign-key-constraint",
    AddingSerialPrimaryKeyField => "adding-serial-primary-key-field",
    DisallowedUniqueConstraint => "disallowed-unique-constraint",
    AddingFieldWithDefault => "adding-field-with-default",
    RequireEnumValueOrdering => "require-enum-value-ordering",
    BanDropType => "ban-drop-type",
    BanDropFunction => "ban-drop-function",
    BanDropSequence => "ban-drop-sequence",
    BanDropView => "ban-drop-view",
    BanDropDefault => "ban-drop-default",
    BanDropNotNull => "ban-drop-not-null",
    RenamingObject => "renaming-object",
    IdentifierTooLong => "identifier-too-long",
    TransactionNesting => "transaction-nesting",
    BanConcurrentIndexCreationInTransaction => "ban-concurrent-index-creation-in-transaction",
    PreferRobustStmts => "prefer-robust-stmts",
    BanDropDatabase => "ban-drop-database",
    BanCharField => "ban-char-field",
    BanTruncateCascade => "ban-truncate-cascade",
    BanDropSchema => "ban-drop-schema",
    BanDropDomain => "ban-drop-domain",
    BanDropExtension => "ban-drop-extension",
    BanSetSchema => "ban-set-schema",
    BanDropGeneratedExpression => "ban-drop-generated-expression",
    BanAlterIdentity => "ban-alter-identity",
    BanCreateDomainWithConstraint => "ban-create-domain-with-constraint",
    BanAlterDomainWithAddConstraint => "ban-alter-domain-with-add-constraint",
    RequireConcurrentReindex => "require-concurrent-reindex",
    RequireConcurrentPartitionDetach => "require-concurrent-partition-detach",
    BanUncommittedTransaction => "ban-uncommitted-transaction",
    BanDuplicateColumnAssignments => "ban-duplicate-column-assignments",
    UnusedIgnore => "unused-ignore",
});

rule_family!(DockerfileRule {
    Dl3000 => "dl3000",
    Dl3002 => "dl3002",
    Dl3003 => "dl3003",
    Dl3004 => "dl3004",
    Dl3006 => "dl3006",
    Dl3007 => "dl3007",
    Dl3008 => "dl3008",
    Dl3009 => "dl3009",
    Dl3012 => "dl3012",
    Dl3013 => "dl3013",
    Dl3014 => "dl3014",
    Dl3015 => "dl3015",
    Dl3016 => "dl3016",
    Dl3018 => "dl3018",
    Dl3019 => "dl3019",
    Dl3020 => "dl3020",
    Dl3021 => "dl3021",
    Dl3022 => "dl3022",
    Dl3023 => "dl3023",
    Dl3024 => "dl3024",
    Dl3025 => "dl3025",
    Dl3027 => "dl3027",
    Dl3029 => "dl3029",
    Dl3032 => "dl3032",
    Dl3042 => "dl3042",
    Dl3045 => "dl3045",
    Dl3047 => "dl3047",
    Dl3059 => "dl3059",
    Dl3061 => "dl3061",
    Dl3062 => "dl3062",
    Dl3064 => "dl3064",
    Dl3066 => "dl3066",
    Dl3067 => "dl3067",
    Dl4000 => "dl4000",
    Dl4001 => "dl4001",
    Dl4003 => "dl4003",
    Dl4004 => "dl4004",
    Dl4006 => "dl4006",
});

rule_family!(PythonRule {
    F401 => "f401",
    F402 => "f402",
    F403 => "f403",
    F404 => "f404",
    F405 => "f405",
    F406 => "f406",
    F501 => "f501",
    F502 => "f502",
    F503 => "f503",
    F504 => "f504",
    F505 => "f505",
    F506 => "f506",
    F507 => "f507",
    F508 => "f508",
    F509 => "f509",
    F521 => "f521",
    F522 => "f522",
    F523 => "f523",
    F524 => "f524",
    F525 => "f525",
    F541 => "f541",
    F601 => "f601",
    F602 => "f602",
    F631 => "f631",
    F632 => "f632",
    F633 => "f633",
    F634 => "f634",
    F701 => "f701",
    F702 => "f702",
    F704 => "f704",
    F706 => "f706",
    F707 => "f707",
    F722 => "f722",
    F811 => "f811",
    F821 => "f821",
    F822 => "f822",
    F823 => "f823",
    F841 => "f841",
    F842 => "f842",
    F901 => "f901",
    E401 => "e401",
    E402 => "e402",
    E701 => "e701",
    E702 => "e702",
    E703 => "e703",
    E711 => "e711",
    E712 => "e712",
    E713 => "e713",
    E714 => "e714",
    E721 => "e721",
    E722 => "e722",
    E731 => "e731",
    E741 => "e741",
    E742 => "e742",
    E743 => "e743",
});

rule_family!(ComposeRule {
    MemoryLimit => "compose-memory-limit",
    HealthcheckStartPeriod => "compose-healthcheck-start-period",
    HealthcheckInterval => "compose-healthcheck-interval",
});

mod eslint;
mod go;
mod shell;

pub use eslint::{EslintRule, eslint_index};
pub use go::{GoRule, go_index};
pub use shell::{ShellRule, shell_index};

pub const FAMILY_RULES: usize = MigrationRule::ALL.len()
    + DockerfileRule::ALL.len()
    + ComposeRule::ALL.len()
    + PythonRule::ALL.len()
    + EslintRule::ALL.len()
    + ShellRule::ALL.len()
    + GoRule::ALL.len();

pub const fn with_families<const CODE: usize, const TOTAL: usize>(code: [RuleId; CODE]) -> [RuleId; TOTAL] {
    let mut all = [RuleId::FileLength; TOTAL];
    let free = copy_rules(&mut all, &code);
    let free = wrap_migrations(free, &MigrationRule::ALL);
    let free = wrap_dockerfile(free, &DockerfileRule::ALL);
    let free = wrap_compose(free, &ComposeRule::ALL);
    let free = wrap_python(free, &PythonRule::ALL);
    let free = eslint::wrap_eslint(free, &EslintRule::ALL);
    let free = shell::wrap_shell(free, &ShellRule::ALL);
    go::wrap_go(free, &GoRule::ALL);
    all
}

const fn copy_rules<'slots>(slots: &'slots mut [RuleId], rules: &[RuleId]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = *rule;
            copy_rules(free, others)
        }
        (free, _) => free,
    }
}

const fn wrap_migrations<'slots>(
    slots: &'slots mut [RuleId],
    rules: &[MigrationRule],
) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Migration(*rule);
            wrap_migrations(free, others)
        }
        (free, _) => free,
    }
}

const fn wrap_dockerfile<'slots>(
    slots: &'slots mut [RuleId],
    rules: &[DockerfileRule],
) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Dockerfile(*rule);
            wrap_dockerfile(free, others)
        }
        (free, _) => free,
    }
}

const fn wrap_compose<'slots>(slots: &'slots mut [RuleId], rules: &[ComposeRule]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Compose(*rule);
            wrap_compose(free, others)
        }
        (free, _) => free,
    }
}

const fn wrap_python<'slots>(slots: &'slots mut [RuleId], rules: &[PythonRule]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Python(*rule);
            wrap_python(free, others)
        }
        (free, _) => free,
    }
}

pub fn migration_index(rule: MigrationRule, code_rules: usize) -> usize {
    code_rules.saturating_add(rule.position())
}

pub fn dockerfile_index(rule: DockerfileRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(rule.position())
}

pub fn compose_index(rule: ComposeRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(rule.position())
}

pub fn python_index(rule: PythonRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(ComposeRule::ALL.len())
        .saturating_add(rule.position())
}

impl Serialize for RuleId {
    fn serialize<Target: Serializer>(&self, serializer: Target) -> Result<Target::Ok, Target::Error> {
        serializer.serialize_str(self.as_str())
    }
}

struct RuleVisitor;

impl Visitor<'_> for RuleVisitor {
    type Value = RuleId;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(EXPECTED_RULE)
    }

    fn visit_str<Failure: de::Error>(self, text: &str) -> Result<RuleId, Failure> {
        text.parse()
            .map_err(|_| Failure::invalid_value(Unexpected::Str(text), &self))
    }
}

impl<'de> Deserialize<'de> for RuleId {
    fn deserialize<Source: Deserializer<'de>>(deserializer: Source) -> Result<Self, Source::Error> {
        deserializer.deserialize_str(RuleVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::{FAMILY_RULES, with_families};
    use crate::rules::RuleId;

    #[test]
    fn every_rule_has_its_own_index_in_order() {
        let built: [RuleId; RuleId::CODE_RULES.len() + FAMILY_RULES] = with_families(RuleId::CODE_RULES);
        assert_eq!(built, RuleId::ALL);
        for (position, rule) in built.into_iter().enumerate() {
            assert_eq!(rule.index(), position, "{rule:?}");
            assert_eq!(rule.as_str().parse::<RuleId>().ok(), Some(rule));
        }
        let error = serde_json::from_str::<RuleId>("7").expect_err("no es una regla");
        assert!(error.to_string().contains("una regla de kerbo"));
    }
}
