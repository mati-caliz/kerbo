use tree_sitter::Node;

use super::model::SourceText;

const SPLITS: [&str; 2] = ["# isort: split", "# ruff: isort: split"];
const SKIP_FILE: [&str; 4] = [
    "# isort: skip_file",
    "# isort:skip_file",
    "# ruff: isort: skip_file",
    "# ruff: isort:skip_file",
];
const ON: [&str; 2] = ["# isort: on", "# ruff: isort: on"];
const OFF: [&str; 2] = ["# isort: off", "# ruff: isort: off"];
const SKIP_MARKERS: [&str; 2] = ["isort: skip", "isort:skip"];

// `extract_isort_directives` de ruff: dónde cortar bloques y qué imports quedan afuera.
#[derive(Debug, Default)]
pub struct Directives {
    pub skip_file: bool,
    pub splits: Vec<usize>,
    pub exclusions: Vec<(usize, usize)>,
}

impl Directives {
    pub fn read(comments: &[Node<'_>], text_source: SourceText<'_>) -> Self {
        let mut directives = Self::default();
        let mut off: Option<usize> = None;
        for comment in comments {
            let start = comment.start_byte();
            let content = text_source.slice(start, comment.end_byte()).trim_end();
            if SPLITS.contains(&content) {
                directives.splits.push(start);
            } else if SKIP_FILE.contains(&content) {
                return Self {
                    skip_file: true,
                    ..Self::default()
                };
            } else if let Some(exclusion_start) = off {
                off = directives.close_exclusion(exclusion_start, start, content);
            } else if SKIP_MARKERS.iter().any(|marker| content.contains(marker)) {
                let line_start = text_source.line_start(start);
                directives
                    .exclusions
                    .push((line_start, text_source.line_end(start)));
            } else if OFF.contains(&content) {
                off = Some(start);
            }
        }
        if let Some(exclusion_start) = off {
            directives
                .exclusions
                .push((exclusion_start, text_source.bytes.len()));
        }
        directives
    }

    // Cierra la exclusión abierta con `# isort: off` si el comentario es un `# isort: on`.
    fn close_exclusion(&mut self, exclusion_start: usize, end: usize, content: &str) -> Option<usize> {
        if ON.contains(&content) {
            self.exclusions.push((exclusion_start, end));
            None
        } else {
            Some(exclusion_start)
        }
    }

    pub fn excludes(&self, offset: usize) -> bool {
        self.exclusions
            .iter()
            .any(|(start, end)| (*start..*end).contains(&offset))
    }
}
