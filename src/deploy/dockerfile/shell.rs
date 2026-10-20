mod words;

use words::{Piece, pieces};

const RESERVED_PREFIXES: &[&str] = &[
    "if", "then", "elif", "else", "do", "while", "until", "!", "{", "fi", "done", "esac", "}", "time",
];
const LOOP_HEADERS: &[&str] = &["for", "select", "case"];
const CONDITIONS: &[&str] = &["[", "[["];
const LONG_FLAG: &str = "--";
const SHORT_FLAG: &str = "-";
const FLAG_VALUE: char = '=';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Script {
    pub commands: Vec<Command>,
    pub has_pipe: bool,
}

impl Script {
    pub fn uses(&self, program: &str) -> bool {
        self.commands.iter().any(|command| command.name == program)
    }

    pub fn any(&self, check: impl Fn(&Command) -> bool) -> bool {
        self.commands.iter().any(check)
    }

    pub fn position(&self, check: impl Fn(&Command) -> bool) -> Option<usize> {
        self.commands.iter().position(check)
    }
}

pub fn parse(script: &str) -> Script {
    let mut parsed = Script::default();
    let mut words: Vec<String> = Vec::new();
    let mut nested: Vec<Command> = Vec::new();
    for piece in pieces(script) {
        match piece {
            Piece::Word(word) => words.push(word),
            Piece::Nested(inner) => {
                parsed.has_pipe |= inner.has_pipe;
                nested.extend(inner.commands);
            }
            Piece::Separator => flush(&mut words, &mut nested, &mut parsed.commands),
            Piece::Pipe => {
                parsed.has_pipe = true;
                flush(&mut words, &mut nested, &mut parsed.commands);
            }
        }
    }
    flush(&mut words, &mut nested, &mut parsed.commands);
    parsed
}

pub fn words(text: &str) -> Vec<String> {
    pieces(text)
        .into_iter()
        .filter_map(|piece| match piece {
            Piece::Word(word) => Some(word),
            Piece::Nested(_) | Piece::Separator | Piece::Pipe => None,
        })
        .collect()
}

fn flush(words: &mut Vec<String>, nested: &mut Vec<Command>, commands: &mut Vec<Command>) {
    command_of(std::mem::take(words), commands);
    commands.append(nested);
}

fn command_of(taken: Vec<String>, commands: &mut Vec<Command>) {
    let mut remaining = taken
        .into_iter()
        .skip_while(|word| RESERVED_PREFIXES.contains(&word.as_str()))
        .skip_while(|word| is_assignment(word));
    let Some(name) = remaining.next() else {
        return;
    };
    if LOOP_HEADERS.contains(&name.as_str()) || CONDITIONS.contains(&name.as_str()) {
        return;
    }
    commands.push(Command {
        name,
        arguments: remaining.collect(),
    });
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once(FLAG_VALUE) else {
        return false;
    };
    let name = name.strip_suffix('+').unwrap_or(name);
    name.starts_with(|first: char| first.is_ascii_alphabetic() || first == '_')
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

impl Command {
    pub fn is(&self, name: &str) -> bool {
        self.name == name
    }

    pub fn has_any_argument(&self, name: &str, wanted: &[&str]) -> bool {
        self.is(name)
            && self
                .arguments
                .iter()
                .any(|argument| wanted.contains(&argument.as_str()))
    }

    pub fn has_argument(&self, wanted: &str) -> bool {
        self.arguments.iter().any(|argument| argument == wanted)
    }

    fn flags(&self) -> Vec<(usize, String)> {
        self.arguments
            .iter()
            .enumerate()
            .flat_map(|(index, argument)| flags_of(argument).into_iter().map(move |flag| (index, flag)))
            .collect()
    }

    pub fn flag_count(&self, flag: &str) -> usize {
        self.flags().iter().filter(|(_, found)| found == flag).count()
    }

    pub fn has_flag(&self, flag: &str) -> bool {
        self.flag_count(flag) > 0
    }

    pub fn has_any_flag(&self, flags: &[&str]) -> bool {
        self.flags()
            .iter()
            .any(|(_, found)| flags.contains(&found.as_str()))
    }

    pub fn arguments_without_flags(&self) -> Vec<&str> {
        let flagged: Vec<usize> = self.flags().into_iter().map(|(index, _)| index).collect();
        self.arguments
            .iter()
            .enumerate()
            .filter(|(index, _)| !flagged.contains(index))
            .map(|(_, argument)| argument.as_str())
            .collect()
    }

    pub fn without_flag_values(&self, flags: &[&str]) -> Self {
        let dropped: Vec<usize> = self
            .flags()
            .into_iter()
            .filter(|(index, flag)| {
                flags.contains(&flag.as_str())
                    && !self
                        .arguments
                        .get(*index)
                        .is_some_and(|argument| argument.contains(FLAG_VALUE))
            })
            .map(|(index, _)| index.saturating_add(1))
            .collect();
        Self {
            name: self.name.clone(),
            arguments: self
                .arguments
                .iter()
                .enumerate()
                .filter(|(index, _)| !dropped.contains(index))
                .map(|(_, argument)| argument.clone())
                .collect(),
        }
    }
}

fn flags_of(argument: &str) -> Vec<String> {
    if argument == LONG_FLAG || argument == SHORT_FLAG {
        return Vec::new();
    }
    if let Some(long) = argument.strip_prefix(LONG_FLAG) {
        return vec![long.split(FLAG_VALUE).next().unwrap_or_default().to_owned()];
    }
    argument
        .strip_prefix(SHORT_FLAG)
        .map(|short| short.chars().map(String::from).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
