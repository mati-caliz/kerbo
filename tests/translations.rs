#[cfg(test)]
mod translations {
    use std::fs;
    use std::path::Path;

    const SPANISH_README: &str = "README.md";
    const ENGLISH_README: &str = "README.en.md";
    const SPANISH_SITE: &str = "site/index.html";
    const ENGLISH_SITE: &str = "site/en/index.html";
    const HEADING_MARK: char = '#';
    const FENCE: &str = "```";
    const TABLE_ROW: &str = "|";
    const SITE_PIECES: [&str; 6] = ["<section", "<h2", "<tr", "<div class=\"install\"", "<pre", "<li"];
    const COPY_ATTRIBUTE: &str = "data-copy=\"";
    const ENGLISH_FLAG: &str = " --lang en";

    #[derive(Debug, PartialEq, Eq)]
    enum Block {
        Heading(usize),
        Code { language: String, lines: usize },
        TableRow,
    }

    fn read(relative: &str) -> String {
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
            .unwrap_or_else(|error| panic!("no se pudo leer {relative}: {error}"))
    }

    fn markdown_shape(text: &str) -> Vec<Block> {
        let mut shape = Vec::new();
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            if let Some(language) = line.strip_prefix(FENCE) {
                let length = lines
                    .by_ref()
                    .take_while(|inner| !inner.starts_with(FENCE))
                    .count();
                shape.push(Block::Code {
                    language: language.to_owned(),
                    lines: length,
                });
            } else if line.starts_with(HEADING_MARK) {
                let level = line
                    .chars()
                    .take_while(|character| *character == HEADING_MARK)
                    .count();
                shape.push(Block::Heading(level));
            } else if line.starts_with(TABLE_ROW) {
                shape.push(Block::TableRow);
            }
        }
        shape
    }

    fn copied_commands(html: &str) -> Vec<String> {
        html.split(COPY_ATTRIBUTE)
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .map(|command| command.replace(ENGLISH_FLAG, ""))
            .collect()
    }

    #[test]
    fn both_readmes_have_the_same_sections_tables_and_code_blocks() {
        assert_eq!(
            markdown_shape(&read(SPANISH_README)),
            markdown_shape(&read(ENGLISH_README)),
            "README.md y README.en.md se desfasaron: un título, una tabla o un bloque de código está en uno solo"
        );
    }

    #[test]
    fn both_sites_have_the_same_pieces_and_commands() {
        let spanish = read(SPANISH_SITE);
        let english = read(ENGLISH_SITE);
        for piece in SITE_PIECES {
            assert_eq!(
                spanish.matches(piece).count(),
                english.matches(piece).count(),
                "el sitio en español y el de inglés tienen distinta cantidad de {piece}"
            );
        }
        assert_eq!(copied_commands(&spanish), copied_commands(&english));
    }

    #[test]
    fn the_shape_counts_headings_tables_and_code() {
        let text = "# a\n\n| x |\n|---|\n\n```sh\nuno\ndos\n```\n## b\n";
        assert_eq!(
            markdown_shape(text),
            [
                Block::Heading(1),
                Block::TableRow,
                Block::TableRow,
                Block::Code {
                    language: "sh".to_owned(),
                    lines: 2,
                },
                Block::Heading(2),
            ]
        );
    }
}
