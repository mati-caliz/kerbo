use super::{
    ComposeRule, DockerfileRule, EslintRule, GoRule, JavaRule, MigrationRule, PythonRule, RuleId, ShellRule,
};

// Lo que llega del usuario sin limpiar a una consulta, un comando, una ruta o la respuesta HTML: las
// mismas reglas para todos los lenguajes.
rule_family!(TaintRule {
    SqlInjection => "sql-injection",
    CommandInjection => "command-injection",
    PathTraversal => "path-traversal",
    CrossSiteScripting => "xss",
});

pub const fn wrap_taint<'slots>(slots: &'slots mut [RuleId], rules: &[TaintRule]) -> &'slots mut [RuleId] {
    match (slots, rules) {
        ([slot, free @ ..], [rule, others @ ..]) => {
            *slot = RuleId::Taint(*rule);
            wrap_taint(free, others)
        }
        (free, _) => free,
    }
}

pub fn taint_index(rule: TaintRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(ComposeRule::ALL.len())
        .saturating_add(PythonRule::ALL.len())
        .saturating_add(EslintRule::ALL.len())
        .saturating_add(ShellRule::ALL.len())
        .saturating_add(GoRule::ALL.len())
        .saturating_add(JavaRule::ALL.len())
        .saturating_add(rule.position())
}
