use clap::ValueEnum;
use serde::Deserialize;

use crate::rules::RuleId;

mod catalog;
mod english;
mod spanish;

pub use catalog::{BlockUnit, Cheat, Explanation, Message, Notice, Problem, Summary, Text};

use catalog::Catalog;

pub const LOCALE_VARIABLES: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];
const ENGLISH_LOCALE_PREFIX: &str = "en";
const LANG_FLAG: &str = "--lang";
const FLAG_VALUE_SEPARATOR: &str = "=";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
pub enum Lang {
    #[default]
    #[value(name = "es")]
    #[serde(rename = "es")]
    Spanish,
    #[value(name = "en")]
    #[serde(rename = "en")]
    English,
}

impl Lang {
    pub fn from_locale(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let locale = LOCALE_VARIABLES
            .iter()
            .filter_map(|variable| lookup(variable))
            .find(|value| !value.is_empty());
        if locale.is_some_and(|value| value.starts_with(ENGLISH_LOCALE_PREFIX)) {
            Self::English
        } else {
            Self::Spanish
        }
    }

    pub fn from_arguments<Argument: AsRef<str>>(arguments: &[Argument]) -> Option<Self> {
        let mut values = arguments.iter().map(AsRef::as_ref);
        let mut found = None;
        while let Some(argument) = values.next() {
            let value = match argument.strip_prefix(LANG_FLAG) {
                Some("") => values.next(),
                Some(rest) => rest.strip_prefix(FLAG_VALUE_SEPARATOR),
                None => None,
            };
            found = value
                .and_then(|value| <Self as ValueEnum>::from_str(value, false).ok())
                .or(found);
        }
        found
    }

    const fn catalog(self) -> &'static dyn Catalog {
        match self {
            Self::Spanish => &spanish::Spanish,
            Self::English => &english::English,
        }
    }

    pub fn text(self, text: Text) -> &'static str {
        self.catalog().text(text)
    }

    pub fn describe(self, rule: RuleId) -> &'static str {
        self.catalog().rule_description(rule)
    }

    pub fn explain(self, rule: RuleId) -> Explanation {
        self.catalog().explanation(rule)
    }

    pub fn message(self, message: &Message) -> String {
        self.catalog().message(message)
    }

    pub fn problem(self, problem: &Problem) -> String {
        self.catalog().problem(problem)
    }

    pub fn notice(self, notice: &Notice) -> String {
        self.catalog().notice(notice)
    }

    pub fn explanation_text(self, rule: RuleId) -> String {
        let explanation = self.explain(rule);
        format!(
            "{}: {}\n\n{}: {}\n{}: {}\n{}: {}",
            rule.as_str(),
            self.describe(rule),
            self.text(Text::WhyLabel),
            explanation.why,
            self.text(Text::FixLabel),
            explanation.fix,
            self.text(Text::ExampleLabel),
            explanation.example
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LangPreference {
    pub flag: Option<Lang>,
    pub locale: Lang,
}

impl LangPreference {
    pub fn resolve(self, configured: Option<Lang>) -> Lang {
        self.flag.or(configured).unwrap_or(self.locale)
    }
}

#[cfg(test)]
mod tests;
