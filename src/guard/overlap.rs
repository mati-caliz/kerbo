use crate::rules::{Diagnostic, EslintRule, RuleId};

const COVERED: [(RuleId, RuleId); 2] = [
    (RuleId::TypeEscape, RuleId::Eslint(EslintRule::NoExplicitAny)),
    (RuleId::SuppressionAdded, RuleId::Eslint(EslintRule::BanTsComment)),
];

pub fn add_guarded(diagnostics: &mut Vec<Diagnostic>, guarded: impl IntoIterator<Item = Diagnostic>) {
    let guarded: Vec<Diagnostic> = guarded.into_iter().collect();
    diagnostics.retain(|diagnostic| {
        !guarded.iter().any(|cheat| {
            cheat.relative_path == diagnostic.relative_path
                && cheat.line == diagnostic.line
                && COVERED.contains(&(cheat.rule, diagnostic.rule))
        })
    });
    diagnostics.extend(guarded);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::add_guarded;
    use crate::i18n::Message;
    use crate::rules::{Diagnostic, EslintRule, RuleId};

    fn diagnostic(rule: RuleId, line: usize) -> Diagnostic {
        Diagnostic {
            path: "a.ts".to_owned(),
            line,
            column: 1,
            end_line: line,
            rule,
            message: Message::Rule(rule),
            value: 1,
            limit: 0,
            fingerprint: String::new(),
            relative_path: "a.ts".to_owned(),
            absolute_path: PathBuf::from("a.ts"),
            fix: None,
        }
    }

    #[test]
    fn the_cheat_replaces_the_lint_on_the_same_line() {
        let mut diagnostics = vec![
            diagnostic(RuleId::Eslint(EslintRule::NoExplicitAny), 3),
            diagnostic(RuleId::Eslint(EslintRule::NoExplicitAny), 4),
            diagnostic(RuleId::Eslint(EslintRule::BanTsComment), 5),
        ];
        add_guarded(
            &mut diagnostics,
            [
                diagnostic(RuleId::TypeEscape, 3),
                diagnostic(RuleId::SuppressionAdded, 5),
            ],
        );
        let left: Vec<(RuleId, usize)> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.rule, diagnostic.line))
            .collect();
        assert_eq!(
            left,
            [
                (RuleId::Eslint(EslintRule::NoExplicitAny), 4),
                (RuleId::TypeEscape, 3),
                (RuleId::SuppressionAdded, 5)
            ]
        );
    }
}
