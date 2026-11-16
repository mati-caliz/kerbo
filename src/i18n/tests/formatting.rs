use super::LANGS;
use crate::formatter::Unparsable;
use crate::i18n::{FormatNotice, Notice};

const LINE: usize = 7;
const PLACE: &str = "main.go:7: ";

#[test]
fn every_reason_to_leave_a_file_unformatted_says_where_and_why_in_both_languages() {
    const CAUSES: [Unparsable; 3] = [
        Unparsable::SyntaxError { line: LINE },
        Unparsable::ReadsUnlikeGo { line: LINE },
        Unparsable::Unsupported { line: LINE },
    ];
    for lang in LANGS {
        let mut texts: Vec<String> = CAUSES
            .iter()
            .map(|cause| {
                lang.notice(&Notice::Format(FormatNotice::Unparsable {
                    path: "main.go".to_owned(),
                    cause: *cause,
                }))
            })
            .collect();
        assert!(texts.iter().all(|text| text.starts_with(PLACE)), "{texts:?}");
        texts.sort();
        texts.dedup();
        assert_eq!(texts.len(), CAUSES.len());
    }
}
