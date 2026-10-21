use std::collections::HashSet;

use super::image::BaseImage;
use super::parser::{Instruction, Keyword};
use super::stages::environment_pairs;
use super::{Findings, Step};
use crate::rules::DockerfileRule;

const MAX_PORT: u32 = 65_535;
const PROTOCOL_SEPARATOR: char = '/';
const RANGE_SEPARATOR: char = '-';
const RESERVED_STAGE_NAMES: &[&str] = &["scratch", "context"];
const REDUNDANT_PLATFORMS: &[&str] = &["$TARGETPLATFORM", "${TARGETPLATFORM}"];
const ARGUMENT_SEPARATORS: [char; 2] = ['=', ' '];
const VARIABLE_MARK: &str = "$";
const LABEL_SYMBOLS: &[char] = &['.', '-', '_', '/'];
const RESERVED_LABEL_NAMESPACES: &[&str] = &["com.docker.", "io.docker.", "org.dockerproject."];
const DOUBLED_SEPARATORS: &[&str] = &["..", "--"];

pub(super) fn check(steps: &[Step], findings: &mut Findings) {
    let mut defined: HashSet<String> = HashSet::new();
    for step in steps {
        let instruction = &step.instruction;
        let line = instruction.line;
        let failed = if instruction.onbuild {
            onbuild_problem(instruction.keyword).into_iter().collect()
        } else {
            plain_problems(instruction, &mut defined)
        };
        for rule in failed {
            findings.report(rule, line, "");
        }
    }
}

const fn onbuild_problem(keyword: Keyword) -> Option<DockerfileRule> {
    match keyword {
        Keyword::Onbuild | Keyword::From | Keyword::Maintainer => Some(DockerfileRule::Dl3043),
        Keyword::Run
        | Keyword::Copy
        | Keyword::Add
        | Keyword::User
        | Keyword::Workdir
        | Keyword::Env
        | Keyword::Arg
        | Keyword::Cmd
        | Keyword::Entrypoint
        | Keyword::Healthcheck
        | Keyword::Shell
        | Keyword::Expose
        | Keyword::Label
        | Keyword::Other => None,
    }
}

fn plain_problems(instruction: &Instruction, defined: &mut HashSet<String>) -> Vec<DockerfileRule> {
    match instruction.keyword {
        Keyword::Expose => invalid_port(&instruction.arguments)
            .then_some(DockerfileRule::Dl3011)
            .into_iter()
            .collect(),
        Keyword::From => stage_problems(instruction),
        Keyword::Label => environment_pairs(&instruction.arguments)
            .iter()
            .any(|(key, _)| invalid_label_key(key))
            .then_some(DockerfileRule::Dl3048)
            .into_iter()
            .collect(),
        Keyword::Env => self_reference(instruction, defined)
            .then_some(DockerfileRule::Dl3044)
            .into_iter()
            .collect(),
        Keyword::Arg => {
            defined.extend(argument_name(instruction));
            Vec::new()
        }
        Keyword::Run
        | Keyword::Copy
        | Keyword::Add
        | Keyword::User
        | Keyword::Workdir
        | Keyword::Cmd
        | Keyword::Entrypoint
        | Keyword::Healthcheck
        | Keyword::Shell
        | Keyword::Maintainer
        | Keyword::Onbuild
        | Keyword::Other => Vec::new(),
    }
}

fn invalid_label_key(key: &str) -> bool {
    let starts_badly = !key.starts_with(|first: char| first.is_ascii_lowercase());
    let ends_badly = !key.ends_with(|last: char| last.is_ascii_lowercase() || last.is_ascii_digit());
    let illegal = key.chars().any(|character| {
        !(character.is_ascii_lowercase() || character.is_ascii_digit() || LABEL_SYMBOLS.contains(&character))
    });
    let reserved = RESERVED_LABEL_NAMESPACES
        .iter()
        .any(|namespace| key.starts_with(namespace));
    let doubled = DOUBLED_SEPARATORS.iter().any(|separator| key.contains(separator));
    starts_badly || ends_badly || illegal || reserved || doubled
}

fn invalid_port(arguments: &str) -> bool {
    arguments.split_whitespace().any(|spec| {
        let port = spec.split(PROTOCOL_SEPARATOR).next().unwrap_or_default();
        let bounds: Vec<Option<u32>> = port
            .split(RANGE_SEPARATOR)
            .map(|bound| bound.parse::<u32>().ok())
            .collect();
        bounds.iter().all(Option::is_some) && bounds.iter().flatten().any(|bound| *bound > MAX_PORT)
    })
}

fn stage_problems(instruction: &Instruction) -> Vec<DockerfileRule> {
    let image = BaseImage::parse(instruction);
    let reserved = image
        .alias
        .as_deref()
        .is_some_and(|alias| RESERVED_STAGE_NAMES.contains(&alias));
    let redundant = image
        .platform
        .as_deref()
        .is_some_and(|platform| REDUNDANT_PLATFORMS.contains(&platform));
    [
        (DockerfileRule::Dl3063, reserved),
        (DockerfileRule::Dl3065, redundant),
    ]
    .into_iter()
    .filter_map(|(rule, failed)| failed.then_some(rule))
    .collect()
}

fn argument_name(instruction: &Instruction) -> Option<String> {
    instruction
        .arguments
        .trim()
        .split(ARGUMENT_SEPARATORS)
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn self_reference(instruction: &Instruction, defined: &mut HashSet<String>) -> bool {
    let pairs = environment_pairs(&instruction.arguments);
    let referenced = pairs.iter().enumerate().any(|(index, (name, _))| {
        !defined.contains(name)
            && pairs
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .any(|(_, (_, value))| references(value, name))
    });
    defined.extend(pairs.into_iter().map(|(name, _)| name));
    referenced
}

fn references(value: &str, name: &str) -> bool {
    if value.contains(&format!("${{{name}}}")) {
        return true;
    }
    let bare = format!("{VARIABLE_MARK}{name}");
    value.contains(&bare)
        && value.split(bare.as_str()).skip(1).any(|rest| {
            !rest
                .chars()
                .next()
                .is_some_and(|next| next.is_ascii_alphanumeric() || next == '_')
        })
}
