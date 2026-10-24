use super::{ComposeRule, DockerfileRule, MigrationRule, PythonRule, RuleId};

// Con un loop y no recursiva: con más de 128 reglas la recursión pasa el límite de llamadas
// anidadas que permite la evaluación en tiempo de compilación.
pub const fn wrap_python<'slots>(
    mut slots: &'slots mut [RuleId],
    mut rules: &[PythonRule],
) -> &'slots mut [RuleId] {
    loop {
        match (slots, rules) {
            ([slot, free @ ..], [rule, others @ ..]) => {
                *slot = RuleId::Python(*rule);
                slots = free;
                rules = others;
            }
            (free, _) => return free,
        }
    }
}

pub fn python_index(rule: PythonRule, code_rules: usize) -> usize {
    code_rules
        .saturating_add(MigrationRule::ALL.len())
        .saturating_add(DockerfileRule::ALL.len())
        .saturating_add(ComposeRule::ALL.len())
        .saturating_add(rule.position())
}
