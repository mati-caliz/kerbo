use crate::rules::ShellRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Style,
}

pub const fn severity(rule: ShellRule) -> Severity {
    match rule {
        ShellRule::Sc2068 | ShellRule::Sc2148 => Severity::Error,
        ShellRule::Sc2034
        | ShellRule::Sc2046
        | ShellRule::Sc2064
        | ShellRule::Sc2069
        | ShellRule::Sc2155
        | ShellRule::Sc2164
        | ShellRule::Sc2166
        | ShellRule::Sc2206
        | ShellRule::Sc2207 => Severity::Warning,
        ShellRule::Sc2015
        | ShellRule::Sc2016
        | ShellRule::Sc2059
        | ShellRule::Sc2086
        | ShellRule::Sc2162
        | ShellRule::Sc2223 => Severity::Info,
        ShellRule::Sc2004 | ShellRule::Sc2005 | ShellRule::Sc2006 | ShellRule::Sc2116 | ShellRule::Sc2181 => {
            Severity::Style
        }
    }
}
