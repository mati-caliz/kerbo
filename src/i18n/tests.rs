use super::{BlockUnit, Cheat, FixNotice, Lang, LangPreference, Message, Notice, Problem, Summary, Text};
use crate::rules::RuleId;

const LANGS: [Lang; 2] = [Lang::Spanish, Lang::English];

const TEXTS: [Text; 34] = [
    Text::WhyLabel,
    Text::FixLabel,
    Text::ExampleLabel,
    Text::McpInstructions,
    Text::McpCheckDescription,
    Text::McpPathsDescription,
    Text::McpDiffDescription,
    Text::McpExplainDescription,
    Text::McpFixDescription,
    Text::McpRulesDescription,
    Text::McpDryRunDescription,
    Text::AboutKerbo,
    Text::AboutCheck,
    Text::AboutBaseline,
    Text::AboutExplain,
    Text::AboutHook,
    Text::AboutMcp,
    Text::AboutLsp,
    Text::AboutFix,
    Text::HelpFixRule,
    Text::HelpDryRun,
    Text::FixAllTitle,
    Text::HelpRule,
    Text::HelpConfig,
    Text::HelpPaths,
    Text::HelpLanguages,
    Text::HelpMax,
    Text::HelpFormat,
    Text::HelpDiff,
    Text::HelpNoBaseline,
    Text::HelpLang,
    Text::ValueFile,
    Text::ValueRuleLimit,
    Text::ValueReference,
];

fn messages() -> Vec<Message> {
    let function = || "process".to_owned();
    vec![
        Message::FileTooLong {
            lines: 900,
            limit: 400,
        },
        Message::FunctionTooLong {
            function: function(),
            lines: 90,
            limit: 80,
        },
        Message::CyclomaticComplexity {
            function: function(),
            value: 12,
            limit: 10,
        },
        Message::CognitiveComplexity {
            function: function(),
            value: 20,
            limit: 15,
        },
        Message::TooManyParameters {
            function: function(),
            count: 5,
            limit: 4,
        },
        Message::NestedTooDeep { depth: 5, limit: 4 },
        Message::UnnamedNumber {
            number: "86400".to_owned(),
        },
        Message::ShortName { name: "x".to_owned() },
        Message::RepeatedFunction { original_line: 3 },
        Message::RepeatedBranch { original_line: 7 },
        Message::DuplicatedBlock {
            size: 120,
            unit: BlockUnit::Tokens,
            copies: "b.ts:4".to_owned(),
            limit: 100,
        },
        Message::DuplicatedBlock {
            size: 30,
            unit: BlockUnit::SyntaxNodes,
            copies: "b.go:4".to_owned(),
            limit: 25,
        },
        Message::Rule(RuleId::CommentedCode),
    ]
    .into_iter()
    .chain(cheats().into_iter().map(Message::Cheat))
    .collect()
}

fn cheats() -> Vec<Cheat> {
    vec![
        Cheat::TestsRemoved {
            removed: 1,
            before: 3,
            after: 2,
            net: 1,
        },
        Cheat::TestsRemoved {
            removed: 2,
            before: 3,
            after: 1,
            net: 2,
        },
        Cheat::EmptyTest {
            name: "it works".to_owned(),
        },
        Cheat::TestSkipped {
            marker: "it.skip".to_owned(),
        },
        Cheat::TypeEscaped {
            marker: "as any".to_owned(),
        },
        Cheat::SuppressionAdded {
            marker: "@ts-ignore".to_owned(),
        },
        Cheat::ThresholdLoosened {
            old: "max = 10".to_owned(),
            new: "max = 20".to_owned(),
        },
    ]
}

fn problems() -> Vec<Problem> {
    let path = || "kerbo.toml".to_owned();
    let detail = || "detalle".to_owned();
    vec![
        Problem::CurrentDirectory,
        Problem::WriteReport(detail()),
        Problem::Unreadable {
            path: path(),
            detail: detail(),
        },
        Problem::MissingReason { path: path() },
        Problem::MaxWithoutRule { path: path() },
        Problem::InvalidPattern {
            path: path(),
            pattern: "a/[".to_owned(),
            detail: detail(),
        },
        Problem::UnknownBaselineVersion {
            path: path(),
            version: 9,
        },
        Problem::GitUnavailable(detail()),
        Problem::GitFailed {
            command: "diff".to_owned(),
            detail: detail(),
        },
        Problem::UnknownRule("nope".to_owned()),
        Problem::ExpectedRuleLimit("nope".to_owned()),
        Problem::NotANonNegativeInteger("-1".to_owned()),
        Problem::ReadEvent(detail()),
        Problem::InvalidEvent(detail()),
        Problem::ReadRequest(detail()),
        Problem::InvalidRequest,
        Problem::UnknownMethod("nope".to_owned()),
        Problem::UnknownTool("nope".to_owned()),
        Problem::InvalidArguments(detail()),
        Problem::ReadEditor(detail()),
        Problem::MissingContentLength,
        Problem::IncompleteMessage(detail()),
        Problem::InvalidMessage(detail()),
    ]
}

fn notices() -> Vec<Notice> {
    vec![
        Notice::Summary(Summary {
            files: 1,
            problems: 1,
            known: 0,
            milliseconds: 5,
            deviations: 0,
            suppressed: 0,
        }),
        Notice::Summary(Summary {
            files: 3,
            problems: 2,
            known: 4,
            milliseconds: 5,
            deviations: 1,
            suppressed: 0,
        }),
        Notice::BaselineSaved {
            count: 1,
            path: "kerbo-baseline.json".to_owned(),
        },
        Notice::HookHeader { count: 2 },
        Notice::Summary(Summary {
            files: 1,
            problems: 0,
            known: 0,
            milliseconds: 5,
            deviations: 0,
            suppressed: 2,
        }),
        Notice::Suppressed {
            path: "Dockerfile".to_owned(),
            line: 4,
            rule: RuleId::FileLength,
            reason: String::new(),
        },
        Notice::Suppressed {
            path: "Dockerfile".to_owned(),
            line: 4,
            rule: RuleId::FileLength,
            reason: "la imagen fija las versiones".to_owned(),
        },
        Notice::Fix(FixNotice::Applied { count: 3, files: 1 }),
        Notice::Fix(FixNotice::Previewed { count: 1, files: 2 }),
        Notice::Fix(FixNotice::Withheld {
            path: "A.java".to_owned(),
            introduced: 1,
        }),
        Notice::Fix(FixNotice::Left { count: 2 }),
    ]
}

#[test]
fn every_text_exists_in_both_languages_and_differs_between_them() {
    for text in TEXTS {
        assert_ne!(Lang::Spanish.text(text), "");
        assert_ne!(Lang::Spanish.text(text), Lang::English.text(text), "{text:?}");
    }
    for message in messages() {
        assert_ne!(
            Lang::Spanish.message(&message),
            Lang::English.message(&message),
            "{message:?}"
        );
    }
    for problem in problems() {
        assert!(
            LANGS.iter().all(|lang| !lang.problem(&problem).is_empty()),
            "{problem:?}"
        );
    }
    for notice in notices() {
        assert_ne!(
            Lang::Spanish.notice(&notice),
            Lang::English.notice(&notice),
            "{notice:?}"
        );
    }
}

#[test]
fn every_rule_is_described_and_explained_in_both_languages() {
    for lang in LANGS {
        for rule in RuleId::ALL {
            let explanation = lang.explain(rule);
            assert!(
                [
                    explanation.fix,
                    explanation.why,
                    explanation.example,
                    lang.describe(rule)
                ]
                .iter()
                .all(|text| !text.is_empty())
            );
            assert!(lang.explanation_text(rule).starts_with(rule.as_str()));
        }
    }
    assert_ne!(
        Lang::Spanish.explain(RuleId::MagicNumber),
        Lang::English.explain(RuleId::MagicNumber)
    );
}

#[test]
fn english_counts_agree_in_number() {
    let english = |notice: &Notice| Lang::English.notice(notice);
    assert_eq!(english(&notices()[0]), "1 file, 1 problem, 5 ms");
    assert_eq!(
        english(&notices()[1]),
        "3 files, 2 problems (and 4 in the baseline), 5 ms · 1 deviation from the preset in kerbo.toml"
    );
    assert_eq!(
        Lang::Spanish.notice(&notices()[1]),
        "3 archivos, 2 problemas (y 4 en la línea de base), 5 ms · 1 desvíos del preset en kerbo.toml"
    );
    assert_eq!(
        Lang::English.message(&Message::Cheat(cheats().remove(0))),
        "1 test was deleted from this file (3 before, 2 now); the whole change has 1 test fewer"
    );
}

#[test]
fn reads_the_language_from_the_locale() {
    let locale = |pairs: &'static [(&'static str, &'static str)]| {
        Lang::from_locale(|variable| {
            pairs
                .iter()
                .find(|(name, _)| *name == variable)
                .map(|(_, value)| (*value).to_owned())
        })
    };
    assert_eq!(locale(&[]), Lang::Spanish);
    assert_eq!(locale(&[("LANG", "en_US.UTF-8")]), Lang::English);
    assert_eq!(locale(&[("LANG", "es_AR.UTF-8")]), Lang::Spanish);
    assert_eq!(locale(&[("LC_ALL", ""), ("LANG", "en_GB")]), Lang::English);
    assert_eq!(
        locale(&[("LC_MESSAGES", "es_AR"), ("LANG", "en_US")]),
        Lang::Spanish,
        "LC_MESSAGES le gana a LANG"
    );
    assert_eq!(locale(&[("LC_ALL", "C")]), Lang::Spanish);
}

#[test]
fn the_flag_wins_over_the_configuration_and_the_locale() {
    assert_eq!(
        Lang::from_arguments(&["kerbo", "--lang", "en", "check"]),
        Some(Lang::English)
    );
    assert_eq!(
        Lang::from_arguments(&["kerbo", "check", "--lang=es"]),
        Some(Lang::Spanish)
    );
    assert_eq!(Lang::from_arguments(&["kerbo", "--lang", "fr"]), None);
    assert_eq!(Lang::from_arguments(&["kerbo", "--lang"]), None);
    let preference = LangPreference {
        flag: None,
        locale: Lang::English,
    };
    assert_eq!(preference.resolve(None), Lang::English);
    assert_eq!(preference.resolve(Some(Lang::Spanish)), Lang::Spanish);
    let flagged = LangPreference {
        flag: Some(Lang::English),
        locale: Lang::Spanish,
    };
    assert_eq!(flagged.resolve(Some(Lang::Spanish)), Lang::English);
}
