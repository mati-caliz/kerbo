use super::{ComposeRule, DockerfileRule, MigrationRule, PythonRule, RuleId};

rule_family!(EslintRule {
    NoVar => "no-var",
    PreferConst => "prefer-const",
    NoUnusedVars => "no-unused-vars",
    NoUnusedExpressions => "no-unused-expressions",
    NoRequireImports => "no-require-imports",
    NoExplicitAny => "no-explicit-any",
    NoNonNullAssertion => "no-non-null-assertion",
    PreferRestParams => "prefer-rest-params",
    NoPrototypeBuiltins => "no-prototype-builtins",
    NoEmpty => "no-empty",
    NoUselessEscape => "no-useless-escape",
    NoCondAssign => "no-cond-assign",
    NoThisAlias => "no-this-alias",
    BanTsComment => "ban-ts-comment",
    NoEmptyObjectType => "no-empty-object-type",
    NoCaseDeclarations => "no-case-declarations",
    NoDebugger => "no-debugger",
    NoDuplicateCase => "no-duplicate-case",
    NoDupeElseIf => "no-dupe-else-if",
    NoDupeKeys => "no-dupe-keys",
    NoRedeclare => "no-redeclare",
    NoFuncAssign => "no-func-assign",
    NoWrapperObjectTypes => "no-wrapper-object-types",
    NoUnsafeFunctionType => "no-unsafe-function-type",
    TripleSlashReference => "triple-slash-reference",
    NoUnsafeFinally => "no-unsafe-finally",
    PreferSpread => "prefer-spread",
    NoConstantCondition => "no-constant-condition",
    NoConstantBinaryExpression => "no-constant-binary-expression",
    NoConstAssign => "no-const-assign",
    NoClassAssign => "no-class-assign",
    NoImportAssign => "no-import-assign",
    NoGlobalAssign => "no-global-assign",
    NoShadowRestrictedNames => "no-shadow-restricted-names",
    NoUnassignedVars => "no-unassigned-vars",
    ValidTypeof => "valid-typeof",
    NoEmptyPattern => "no-empty-pattern",
    NoSparseArrays => "no-sparse-arrays",
    RequireYield => "require-yield",
    NoUselessCatch => "no-useless-catch",
    NoCompareNegZero => "no-compare-neg-zero",
    NoAsyncPromiseExecutor => "no-async-promise-executor",
    UseIsnan => "use-isnan",
    NoUnusedLabels => "no-unused-labels",
    NoSelfAssign => "no-self-assign",
    NoSetterReturn => "no-setter-return",
    NoUnsafeOptionalChaining => "no-unsafe-optional-chaining",
    NoNonNullAssertedOptionalChain => "no-non-null-asserted-optional-chain",
    PreserveCaughtError => "preserve-caught-error",
    NoExtraBooleanCast => "no-extra-boolean-cast",
    NoNamespace => "no-namespace",
    NoArrayConstructor => "no-array-constructor",
});

pub const fn wrap_eslint<'slots>(slots: &'slots mut [RuleId], rules: &[EslintRule]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Eslint(*rule);
            wrap_eslint(free, others)
        }
        (free, _) => free,
    }
}

pub fn eslint_index(rule: EslintRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(ComposeRule::ALL.len())
        .saturating_add(PythonRule::ALL.len())
        .saturating_add(rule.position())
}
