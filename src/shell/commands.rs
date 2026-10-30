use tree_sitter::Node;

use super::syntax::{
    ARGUMENT_FIELD, ASSIGNMENT, COMMAND, DECLARATION, NAME_FIELD, UNSET, children, field_children,
    named_children, text,
};
use super::words::{literal, literal_or, oversimplify};

const FLAG_PREFIX: char = '-';
const LONG_FLAG_PREFIX: &str = "--";
const END_OF_FLAGS: &str = "--";
const LONG_FLAG_VALUE: char = '=';
const NOT_LITERAL: &str = "\0";
const NUL: char = '\0';
const TAKES_ARGUMENT: char = ':';
const FIRST_ARGUMENT_COMMANDS: &[&str] = &["busybox", "builtin", "command", "run"];
const EXEC: &str = "exec";
const EXEC_OPTIONS: &str = "cla:";
const LINE_FEED: u8 = b'\n';

#[derive(Debug, Clone)]
pub struct Command<'tree> {
    pub node: Node<'tree>,
    pub words: Vec<Node<'tree>>,
    pub assignments: Vec<Node<'tree>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Opt<'tree> {
    pub flag: Node<'tree>,
    pub value: Node<'tree>,
}

impl<'tree> Command<'tree> {
    pub fn of(node: Node<'tree>) -> Option<Self> {
        match node.kind() {
            COMMAND => {
                let name = node
                    .child_by_field_name(NAME_FIELD)
                    .and_then(|name| name.named_child(0));
                let mut words: Vec<Node<'tree>> = name.into_iter().collect();
                words.extend(field_children(node, ARGUMENT_FIELD));
                let assignments = named_children(node)
                    .into_iter()
                    .filter(|child| child.kind() == ASSIGNMENT)
                    .collect();
                Some(Self {
                    node,
                    words,
                    assignments,
                })
            }
            DECLARATION | UNSET => {
                let keyword = children(node).into_iter().next()?;
                let mut words = vec![keyword];
                words.extend(named_children(node));
                Some(Self {
                    node,
                    words,
                    assignments: Vec::new(),
                })
            }
            _ => None,
        }
    }

    pub fn first_word(&self) -> Option<Node<'tree>> {
        self.words.first().copied()
    }

    pub fn arguments(&self) -> &[Node<'tree>] {
        self.words.get(1..).unwrap_or_default()
    }

    pub fn literal_name(&self, source: &[u8]) -> Option<String> {
        let first = self.first_word()?;
        if !first.is_named() {
            return Some(text(first, source).to_owned());
        }
        literal(first, source)
    }

    pub fn name(&self, source: &[u8]) -> Option<String> {
        self.name_and_word(source).0
    }

    pub fn name_word(&self, source: &[u8]) -> Option<Node<'tree>> {
        self.name_and_word(source).1
    }

    fn name_and_word(&self, source: &[u8]) -> (Option<String>, Option<Node<'tree>>) {
        let Some(first) = self.first_word() else {
            return (None, None);
        };
        let Some(name) = self.literal_name(source) else {
            return (None, Some(first));
        };
        self.effective_word(&name, source)
            .map_or((Some(name), Some(first)), |effective| {
                (literal(effective, source), Some(effective))
            })
    }

    fn effective_word(&self, name: &str, source: &[u8]) -> Option<Node<'tree>> {
        let arguments = self.arguments();
        if FIRST_ARGUMENT_COMMANDS.contains(&name) {
            let first = *arguments.first()?;
            return (!is_flag(first, source)).then_some(first);
        }
        if name == EXEC {
            return bsd_options(EXEC_OPTIONS, arguments, source)?
                .into_iter()
                .find(|(flag, _)| flag.is_empty())
                .map(|(_, opt)| opt.value);
        }
        None
    }

    pub fn is_exactly(&self, expected: &str, source: &[u8]) -> bool {
        self.name(source).is_some_and(|name| name == expected)
    }

    pub fn flags(&self, source: &[u8]) -> Vec<(Node<'tree>, String)> {
        flags_of(self.arguments(), source)
    }

    pub fn has_flag(&self, flag: &str, source: &[u8]) -> bool {
        self.flags(source).iter().any(|(_, name)| name == flag)
    }
}

// tree-sitter-bash pega a veces el comando de la línea siguiente (un `\.` al principio) como una
// palabra que empieza con el salto de línea: cada tramo es uno de los comandos que ve ShellCheck.
pub fn merged_commands<'words, 'tree>(
    words: &'words [Node<'tree>],
    source: &[u8],
) -> Vec<&'words [Node<'tree>]> {
    let mut segments = Vec::new();
    let mut start = 0;
    for (index, word) in words.iter().enumerate().skip(1) {
        if source.get(word.start_byte()) == Some(&LINE_FEED) {
            segments.push(words.get(start..index).unwrap_or_default());
            start = index;
        }
    }
    segments.push(words.get(start..).unwrap_or_default());
    segments
}

// `getAllFlags` de ShellCheck: cada letra de `-abc`, el nombre de `--largo=valor` y "" para lo que
// no es opción o viene después de `--`.
pub fn flags_of<'tree>(arguments: &[Node<'tree>], source: &[u8]) -> Vec<(Node<'tree>, String)> {
    let mut flags = Vec::new();
    let mut finished = false;
    for &argument in arguments {
        let simplified = oversimplify(argument, source);
        finished |= simplified == END_OF_FLAGS;
        if finished {
            flags.push((argument, String::new()));
        } else if let Some(long) = simplified.strip_prefix(LONG_FLAG_PREFIX) {
            let name = long.split(LONG_FLAG_VALUE).next().unwrap_or_default();
            flags.push((argument, name.to_owned()));
        } else if let Some(short) = simplified.strip_prefix(FLAG_PREFIX) {
            flags.extend(short.chars().map(|flag| (argument, flag.to_string())));
        } else {
            flags.push((argument, String::new()));
        }
    }
    flags
}

pub fn is_flag(word: Node<'_>, source: &[u8]) -> bool {
    super::words::parts(word)
        .first()
        .is_some_and(|first| literal(*first, source).is_some_and(|text| text.starts_with(FLAG_PREFIX)))
}

pub fn gnu_options<'tree>(
    spec: &str,
    arguments: &[Node<'tree>],
    source: &[u8],
) -> Option<Vec<(String, Opt<'tree>)>> {
    options(spec, arguments, source, true)
}

pub fn bsd_options<'tree>(
    spec: &str,
    arguments: &[Node<'tree>],
    source: &[u8],
) -> Option<Vec<(String, Opt<'tree>)>> {
    options(spec, arguments, source, false)
}

fn options<'tree>(
    spec: &str,
    arguments: &[Node<'tree>],
    source: &[u8],
    gnu: bool,
) -> Option<Vec<(String, Opt<'tree>)>> {
    let takes_argument = |flag: char| -> Option<bool> {
        let position = spec.find(flag)?;
        Some(
            spec.get(position.saturating_add(1)..)?
                .starts_with(TAKES_ARGUMENT),
        )
    };
    let mut parsed = Vec::new();
    let mut index = 0;
    while let Some(&token) = arguments.get(index) {
        index = index.saturating_add(1);
        let text = literal_or(token, source, NOT_LITERAL);
        if text == END_OF_FLAGS {
            parsed.extend(rest(arguments.get(index..).unwrap_or_default()));
            return Some(parsed);
        }
        if text.starts_with(LONG_FLAG_PREFIX) {
            return None;
        }
        let Some(short) = text.strip_prefix(FLAG_PREFIX) else {
            if !gnu {
                parsed.extend(rest(arguments.get(index.saturating_sub(1)..).unwrap_or_default()));
                return Some(parsed);
            }
            parsed.push((
                String::new(),
                Opt {
                    flag: token,
                    value: token,
                },
            ));
            continue;
        };
        let mut letters = short.chars().peekable();
        while let Some(letter) = letters.next() {
            let value = if takes_argument(letter)? && letters.peek().is_none() {
                let next = *arguments.get(index)?;
                index = index.saturating_add(1);
                next
            } else {
                token
            };
            parsed.push((letter.to_string(), Opt { flag: token, value }));
            if takes_argument(letter)? {
                break;
            }
        }
    }
    Some(parsed)
}

fn rest<'tree, 'list>(arguments: &'list [Node<'tree>]) -> impl Iterator<Item = (String, Opt<'tree>)> + 'list {
    arguments
        .iter()
        .map(|&value| (String::new(), Opt { flag: value, value }))
}

pub fn generic_options<'tree>(arguments: &[Node<'tree>], source: &[u8]) -> Vec<(String, Opt<'tree>)> {
    let mut parsed = Vec::new();
    let mut index = 0;
    while let Some(&token) = arguments.get(index) {
        index = index.saturating_add(1);
        let text = literal_or(token, source, NOT_LITERAL);
        if text == END_OF_FLAGS {
            parsed.extend(rest(arguments.get(index..).unwrap_or_default()));
            break;
        }
        if let Some(long) = text.strip_prefix(LONG_FLAG_PREFIX) {
            let name: String = long
                .chars()
                .take_while(|letter| ![NUL, LONG_FLAG_VALUE].contains(letter))
                .collect();
            parsed.push((
                name,
                Opt {
                    flag: token,
                    value: token,
                },
            ));
            continue;
        }
        let Some(short) = text.strip_prefix(FLAG_PREFIX) else {
            parsed.push((
                String::new(),
                Opt {
                    flag: token,
                    value: token,
                },
            ));
            continue;
        };
        let letters: Vec<char> = short.chars().take_while(|letter| *letter != NUL).collect();
        let flag_of = |letter: &char, value| (letter.to_string(), Opt { flag: token, value });
        match arguments.get(index).copied() {
            Some(next) if !literal_or(next, source, NOT_LITERAL).starts_with(FLAG_PREFIX) => {
                index = index.saturating_add(1);
                if let Some((last, initial)) = letters.split_last() {
                    parsed.extend(initial.iter().map(|letter| flag_of(letter, token)));
                    parsed.push(flag_of(last, next));
                }
            }
            _ => parsed.extend(letters.iter().map(|letter| flag_of(letter, token))),
        }
    }
    parsed
}
