const DEFAULT_ESCAPE: char = '\\';
const ESCAPE_DIRECTIVE: &str = "escape=";
const COMMENT: char = '#';
const FLAG_PREFIX: &str = "--";
const FLAG_VALUE: char = '=';
const HEREDOC_MARKER: &str = "<<";
const HEREDOC_STRIP_TABS: char = '-';
const HEREDOC_QUOTES: &[char] = &['"', '\''];
const JSON_OPEN: char = '[';
const ONBUILD: &str = "ONBUILD";
const HEREDOC_KEYWORDS: &[Keyword] = &[Keyword::Run, Keyword::Copy, Keyword::Add];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    From,
    Run,
    Copy,
    Add,
    User,
    Workdir,
    Env,
    Arg,
    Cmd,
    Entrypoint,
    Healthcheck,
    Shell,
    Maintainer,
    Expose,
    Onbuild,
    Label,
    Other,
}

impl Keyword {
    fn parse(word: &str) -> Self {
        match word.to_uppercase().as_str() {
            "FROM" => Self::From,
            "RUN" => Self::Run,
            "COPY" => Self::Copy,
            "ADD" => Self::Add,
            "USER" => Self::User,
            "WORKDIR" => Self::Workdir,
            "ENV" => Self::Env,
            "ARG" => Self::Arg,
            "CMD" => Self::Cmd,
            "ENTRYPOINT" => Self::Entrypoint,
            "HEALTHCHECK" => Self::Healthcheck,
            "SHELL" => Self::Shell,
            "MAINTAINER" => Self::Maintainer,
            "EXPOSE" => Self::Expose,
            "ONBUILD" => Self::Onbuild,
            "LABEL" => Self::Label,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub line: usize,
    pub keyword: Keyword,
    pub onbuild: bool,
    pub flags: Vec<Flag>,
    pub arguments: String,
    pub heredoc: Option<String>,
}

impl Instruction {
    pub fn flag(&self, name: &str) -> Option<&str> {
        self.flags
            .iter()
            .find(|flag| flag.name == name)
            .map(|flag| flag.value.as_str())
    }

    pub fn exec_form(&self) -> Option<Vec<String>> {
        let text = self.arguments.trim();
        if !text.starts_with(JSON_OPEN) {
            return None;
        }
        serde_json::from_str(text).ok()
    }

    pub fn script(&self) -> String {
        if let Some(body) = self
            .heredoc
            .as_ref()
            .filter(|_| self.arguments.trim_start().starts_with(HEREDOC_MARKER))
        {
            return body.clone();
        }
        let base = self
            .exec_form()
            .map_or_else(|| self.arguments.clone(), |words| words.join(" "));
        match &self.heredoc {
            Some(body) => format!("{base}\n{body}"),
            None => base,
        }
    }
}

struct LineReader<'source> {
    lines: Vec<&'source str>,
    next: usize,
    escape: char,
}

pub fn parse(source: &str) -> Vec<Instruction> {
    let lines: Vec<&str> = source
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let escape = escape_directive(&lines);
    let mut reader = LineReader {
        lines,
        next: 0,
        escape,
    };
    let mut instructions = Vec::new();
    while let Some((line, text)) = reader.logical_line() {
        instructions.extend(reader.instruction(line, &text));
    }
    instructions
}

fn escape_directive(lines: &[&str]) -> char {
    lines
        .iter()
        .map_while(|line| line.trim().strip_prefix(COMMENT))
        .find_map(|directive| {
            let compact: String = directive
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            compact
                .to_lowercase()
                .strip_prefix(ESCAPE_DIRECTIVE)
                .and_then(|value| value.chars().next())
        })
        .unwrap_or(DEFAULT_ESCAPE)
}

fn is_skippable(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.is_empty() || trimmed.starts_with(COMMENT)
}

impl<'source> LineReader<'source> {
    fn take(&mut self) -> Option<(usize, &'source str)> {
        let line = *self.lines.get(self.next)?;
        self.next = self.next.saturating_add(1);
        Some((self.next, line))
    }

    fn logical_line(&mut self) -> Option<(usize, String)> {
        let (number, first) = loop {
            let (number, line) = self.take()?;
            if !is_skippable(line) {
                break (number, line);
            }
        };
        let mut text = first.to_owned();
        while let Some(stripped) = text.trim_end().strip_suffix(self.escape).map(str::to_owned) {
            text = stripped;
            let Some(continuation) = self.next_continuation() else {
                break;
            };
            text.push(' ');
            text.push_str(continuation);
        }
        Some((number, text))
    }

    fn next_continuation(&mut self) -> Option<&'source str> {
        loop {
            let (_, line) = self.take()?;
            if !is_skippable(line) {
                return Some(line);
            }
        }
    }

    fn instruction(&mut self, line: usize, text: &str) -> Option<Instruction> {
        let (word, rest) = split_word(text.trim_start());
        let (keyword, onbuild, rest) = if word.eq_ignore_ascii_case(ONBUILD) {
            let (inner, inner_rest) = split_word(rest);
            (Keyword::parse(inner), true, inner_rest)
        } else {
            (Keyword::parse(word), false, rest)
        };
        if word.is_empty() {
            return None;
        }
        let (flags, arguments) = leading_flags(rest);
        let delimiters = if HEREDOC_KEYWORDS.contains(&keyword) {
            heredoc_delimiters(arguments)
        } else {
            Vec::new()
        };
        let heredoc = (!delimiters.is_empty()).then(|| self.heredoc_body(&delimiters));
        Some(Instruction {
            line,
            keyword,
            onbuild,
            flags,
            arguments: arguments.to_owned(),
            heredoc,
        })
    }

    fn heredoc_body(&mut self, delimiters: &[(String, bool)]) -> String {
        let mut body = Vec::new();
        for (delimiter, strips_tabs) in delimiters {
            self.heredoc_lines(delimiter, *strips_tabs, &mut body);
        }
        body.join("\n")
    }

    fn heredoc_lines(&mut self, delimiter: &str, strips_tabs: bool, body: &mut Vec<String>) {
        while let Some((_, line)) = self.take() {
            let candidate = if strips_tabs {
                line.trim_start_matches('\t')
            } else {
                line
            };
            if candidate == delimiter {
                return;
            }
            body.push(line.to_owned());
        }
    }
}

fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    text.split_once(char::is_whitespace)
        .map_or((text, ""), |(word, rest)| (word, rest.trim_start()))
}

fn leading_flags(text: &str) -> (Vec<Flag>, &str) {
    let mut flags = Vec::new();
    let mut rest = text;
    while rest.starts_with(FLAG_PREFIX) {
        let (word, after) = split_word(rest);
        let body = word.trim_start_matches(FLAG_PREFIX);
        let (name, value) = body.split_once(FLAG_VALUE).unwrap_or((body, ""));
        flags.push(Flag {
            name: name.to_lowercase(),
            value: value.to_owned(),
        });
        rest = after;
    }
    (flags, rest)
}

fn heredoc_delimiters(arguments: &str) -> Vec<(String, bool)> {
    arguments
        .split_whitespace()
        .filter_map(|word| word.strip_prefix(HEREDOC_MARKER))
        .filter(|marker| !marker.starts_with('<'))
        .filter_map(|marker| {
            let strips_tabs = marker.starts_with(HEREDOC_STRIP_TABS);
            let name = marker.trim_start_matches(HEREDOC_STRIP_TABS);
            let name = name.trim_matches(HEREDOC_QUOTES);
            let name: String = name
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            (!name.is_empty()).then_some((name, strips_tabs))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Keyword, parse};

    #[test]
    fn joins_continuations_and_skips_comments_inside_them() {
        let instructions = parse(
            "FROM debian\n# comment\nRUN apt-get update \\\n# inner\n  && apt-get install -y curl\nUSER app\n",
        );
        assert_eq!(instructions.len(), 3);
        assert_eq!(instructions[1].keyword, Keyword::Run);
        assert_eq!(instructions[1].line, 3);
        assert_eq!(
            instructions[1].arguments,
            "apt-get update    && apt-get install -y curl"
        );
        assert_eq!(instructions[2].line, 6);
    }

    #[test]
    fn reads_flags_heredocs_onbuild_and_the_escape_directive() {
        let instructions = parse(
            "# escape=`\nFROM --platform=linux/amd64 alpine AS base\nRUN --mount=type=cache,target=/root <<EOF\napk add curl\nEOF\nONBUILD RUN make `\n  all\nCMD [\"run\"]\n",
        );
        assert_eq!(instructions[0].flag("platform"), Some("linux/amd64"));
        assert_eq!(instructions[1].flag("mount"), Some("type=cache,target=/root"));
        assert_eq!(instructions[1].script(), "apk add curl");
        assert!(instructions[2].onbuild);
        assert_eq!(instructions[2].keyword, Keyword::Run);
        assert_eq!(instructions[2].arguments, "make    all");
        assert_eq!(instructions[3].exec_form(), Some(vec!["run".to_owned()]));
    }
}
