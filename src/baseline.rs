use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::i18n::Problem;
use crate::rules::Diagnostic;

pub const BASELINE_FILE: &str = "kerbo-baseline.json";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct BaselineFile {
    version: u32,
    problems: Vec<KnownProblem>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KnownProblem {
    fingerprint: String,
    value: usize,
}

#[derive(Debug, Default)]
pub struct Baseline {
    known: HashMap<String, usize>,
}

#[derive(Debug)]
pub struct Filtered {
    pub diagnostics: Vec<Diagnostic>,
    pub known: usize,
}

impl Baseline {
    pub fn load(path: &Path) -> Result<Option<Self>, Problem> {
        let unreadable = |detail: String| Problem::Unreadable {
            path: path.display().to_string(),
            detail,
        };
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(unreadable(error.to_string())),
        };
        let file: BaselineFile =
            serde_json::from_str(&text).map_err(|error| unreadable(error.to_string()))?;
        if file.version != FORMAT_VERSION {
            return Err(Problem::UnknownBaselineVersion {
                path: path.display().to_string(),
                version: file.version,
            });
        }
        let known = file
            .problems
            .into_iter()
            .map(|problem| (problem.fingerprint, problem.value))
            .collect();
        Ok(Some(Self { known }))
    }

    pub fn save(diagnostics: &[Diagnostic], path: &Path) -> io::Result<usize> {
        let mut problems: Vec<KnownProblem> = diagnostics
            .iter()
            .map(|diagnostic| KnownProblem {
                fingerprint: diagnostic.fingerprint.clone(),
                value: diagnostic.value,
            })
            .collect();
        problems.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));
        let count = problems.len();
        let mut text = serde_json::to_string_pretty(&BaselineFile {
            version: FORMAT_VERSION,
            problems,
        })?;
        text.push('\n');
        fs::write(path, text)?;
        Ok(count)
    }

    pub fn filter(&self, diagnostics: Vec<Diagnostic>) -> Filtered {
        let (known, new): (Vec<Diagnostic>, Vec<Diagnostic>) =
            diagnostics.into_iter().partition(|diagnostic| {
                self.known
                    .get(&diagnostic.fingerprint)
                    .is_some_and(|&accepted| diagnostic.value <= accepted)
            });
        Filtered {
            diagnostics: new,
            known: known.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{BASELINE_FILE, Baseline};
    use crate::i18n::Message;
    use crate::rules::{Diagnostic, RuleId};
    use crate::test_support::TempTree;

    fn diagnostic(fingerprint: &str, value: usize) -> Diagnostic {
        Diagnostic {
            path: "a.go".to_owned(),
            line: 1,
            column: 1,
            end_line: 2,
            rule: RuleId::CyclomaticComplexity,
            message: Message::Rule(RuleId::CyclomaticComplexity),
            value,
            limit: 10,
            fingerprint: fingerprint.to_owned(),
            relative_path: "a.go".to_owned(),
            absolute_path: PathBuf::from("a.go"),
        }
    }

    #[test]
    fn hides_known_problems_unless_they_got_worse() {
        let tree = TempTree::new("baseline");
        let path = tree.path().join(BASELINE_FILE);
        let saved = Baseline::save(&[diagnostic("b", 12), diagnostic("a", 11)], &path).expect("se escribe");
        assert_eq!(saved, 2);
        let baseline = Baseline::load(&path).expect("se lee").expect("existe");
        let filtered = baseline.filter(vec![
            diagnostic("a", 11),
            diagnostic("b", 13),
            diagnostic("c", 11),
        ]);
        let reported: Vec<&str> = filtered
            .diagnostics
            .iter()
            .map(|kept| kept.fingerprint.as_str())
            .collect();
        assert_eq!(reported, ["b", "c"], "b empeoró y c es nuevo");
        assert_eq!(filtered.known, 1);
    }

    #[test]
    fn missing_file_means_no_baseline_and_bad_files_fail() {
        let tree = TempTree::new("baseline-bad");
        assert!(
            Baseline::load(&tree.path().join(BASELINE_FILE))
                .expect("no falla")
                .is_none()
        );
        tree.write("broken.json", "{");
        assert!(Baseline::load(&tree.path().join("broken.json")).is_err());
        tree.write("future.json", "{\"version\": 9, \"problems\": []}");
        assert!(Baseline::load(&tree.path().join("future.json")).is_err());
        assert!(
            Baseline::load(tree.path()).is_err(),
            "una carpeta no es un archivo"
        );
        assert_eq!(
            Baseline::default()
                .filter(vec![diagnostic("x", 20)])
                .diagnostics
                .len(),
            1
        );
    }
}
