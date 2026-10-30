use crate::rules::ShellRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Style,
}

const ERRORS: &[ShellRule] = &[
    ShellRule::Sc2068,
    ShellRule::Sc2148,
    ShellRule::Sc2224,
    ShellRule::Sc2225,
    ShellRule::Sc2304,
    ShellRule::Sc2142,
    ShellRule::Sc2151,
    ShellRule::Sc2152,
    ShellRule::Sc2168,
    ShellRule::Sc2173,
    ShellRule::Sc2241,
    ShellRule::Sc2242,
];
const INFOS: &[ShellRule] = &[
    ShellRule::Sc2015,
    ShellRule::Sc2016,
    ShellRule::Sc2059,
    ShellRule::Sc2086,
    ShellRule::Sc2162,
    ShellRule::Sc2223,
    ShellRule::Sc2018,
    ShellRule::Sc2019,
    ShellRule::Sc2020,
    ShellRule::Sc2021,
    ShellRule::Sc2022,
    ShellRule::Sc2023,
    ShellRule::Sc2028,
    ShellRule::Sc2029,
    ShellRule::Sc2117,
    ShellRule::Sc2185,
    ShellRule::Sc2196,
    ShellRule::Sc2197,
    ShellRule::Sc2267,
    ShellRule::Sc2308,
];
const STYLES: &[ShellRule] = &[
    ShellRule::Sc2004,
    ShellRule::Sc2005,
    ShellRule::Sc2006,
    ShellRule::Sc2116,
    ShellRule::Sc2181,
    ShellRule::Sc2003,
    ShellRule::Sc2219,
    ShellRule::Sc2293,
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
