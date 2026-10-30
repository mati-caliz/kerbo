use crate::rules::ShellRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Style,
}

const ERRORS: &[ShellRule] = &[ShellRule::Sc2068, ShellRule::Sc2148];
const INFOS: &[ShellRule] = &[
    ShellRule::Sc2015,
    ShellRule::Sc2016,
    ShellRule::Sc2059,
    ShellRule::Sc2086,
    ShellRule::Sc2162,
    ShellRule::Sc2223,
];
const STYLES: &[ShellRule] = &[
    ShellRule::Sc2004,
    ShellRule::Sc2005,
    ShellRule::Sc2006,
    ShellRule::Sc2116,
    ShellRule::Sc2181,
];

// La severidad de ShellCheck; las que no están en ninguna lista son advertencias, como todas las de
// `checkBashisms` (en dash ShellCheck las sube a error, pero eso no cambia qué preset las muestra).
pub fn severity(rule: ShellRule) -> Severity {
    if ERRORS.contains(&rule) {
        Severity::Error
    } else if INFOS.contains(&rule) {
        Severity::Info
    } else if STYLES.contains(&rule) {
        Severity::Style
    } else {
        Severity::Warning
    }
}
