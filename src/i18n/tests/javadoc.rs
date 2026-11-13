use super::LANGS;
use crate::i18n::Message;
use crate::lint::{JavadocTagProblem, LintKind};
use crate::rules::JavaRule;

#[test]
fn every_javadoc_tag_problem_names_the_tag_in_both_languages() {
    const PROBLEMS: [JavadocTagProblem; 5] = [
        JavadocTagProblem::Missing,
        JavadocTagProblem::Unused,
        JavadocTagProblem::Duplicate,
        JavadocTagProblem::Unknown,
        JavadocTagProblem::InvalidInheritDoc,
    ];
    const TAG: &str = "@param <T>";
    for lang in LANGS {
        let texts: Vec<String> = PROBLEMS
            .into_iter()
            .map(|problem| {
                lang.message(&Message::Lint {
                    kind: LintKind::Javadoc(JavaRule::JavadocMethod, problem),
                    subject: TAG.to_owned(),
                })
            })
            .collect();
        assert!(texts.iter().all(|text| text.contains(TAG)), "{lang:?}: {texts:?}");
        let mut distinct = texts.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(
            distinct.len(),
            PROBLEMS.len(),
            "{lang:?}: cada problema dice algo distinto"
        );
    }
}
