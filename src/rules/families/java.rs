use super::{ComposeRule, DockerfileRule, EslintRule, GoRule, MigrationRule, PythonRule, RuleId, ShellRule};

rule_family!(JavaRule {
    TypeName => "type-name",
    MethodName => "method-name",
    ConstantName => "constant-name",
    StaticVariableName => "static-variable-name",
    MemberName => "member-name",
    ParameterName => "parameter-name",
    UpperEll => "upper-ell",
    EmptyStatement => "empty-statement",
    ModifierOrder => "modifier-order",
    ArrayTypeStyle => "array-type-style",
    MultipleVariableDeclarations => "multiple-variable-declarations",
    InterfaceIsType => "interface-is-type",
    MissingSwitchDefault => "missing-switch-default",
    NeedBraces => "need-braces",
    EmptyBlock => "empty-block",
    AvoidNestedBlocks => "avoid-nested-blocks",
    InnerAssignment => "inner-assignment",
    SimplifyBooleanExpression => "simplify-boolean-expression",
    EqualsHashCode => "equals-hash-code",
    HideUtilityClassConstructor => "hide-utility-class-constructor",
    TodoComment => "todo-comment",
    IllegalImport => "illegal-import",
    FileTabCharacter => "file-tab-character",
    TrailingSpaces => "trailing-spaces",
    NewlineAtEndOfFile => "newline-at-end-of-file",
    DesignForExtension => "design-for-extension",
    HiddenField => "hidden-field",
    VisibilityModifier => "visibility-modifier",
    DeclarationOrder => "declaration-order",
    LocalFinalVariableName => "local-final-variable-name",
    WhitespaceAround => "whitespace-around",
    WhitespaceAfter => "whitespace-after",
    NoWhitespaceBefore => "no-whitespace-before",
    NoWhitespaceAfter => "no-whitespace-after",
    GenericWhitespace => "generic-whitespace",
    MethodParamPad => "method-param-pad",
    EmptyForIteratorPad => "empty-for-iterator-pad",
    LeftCurly => "left-curly",
    RightCurly => "right-curly",
    OperatorWrap => "operator-wrap",
});

pub const fn wrap_java<'slots>(slots: &'slots mut [RuleId], rules: &[JavaRule]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Java(*rule);
            wrap_java(free, others)
        }
        (free, _) => free,
    }
}

pub fn java_index(rule: JavaRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(ComposeRule::ALL.len())
        .saturating_add(PythonRule::ALL.len())
        .saturating_add(EslintRule::ALL.len())
        .saturating_add(ShellRule::ALL.len())
        .saturating_add(GoRule::ALL.len())
        .saturating_add(rule.position())
}
