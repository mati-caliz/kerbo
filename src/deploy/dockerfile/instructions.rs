use super::image::BaseImage;
use super::parser::{Instruction, Keyword};
use super::{Findings, Step, unquoted};
use crate::rules::DockerfileRule;

const VARIABLE_PREFIX: &str = "$";
const ROOT: &str = "/";
const LATEST: &str = "latest";
const PLATFORM_VARIABLES: &[&str] = &["BUILDPLATFORM", "TARGETPLATFORM"];
const URL_PREFIXES: &[&str] = &["https://", "http://"];
const ARCHIVE_EXTENSIONS: &[&str] = &[
    ".tar", ".Z", ".bz2", ".gz", ".lz", ".lzma", ".tZ", ".tb2", ".tbz", ".tbz2", ".tgz", ".tlz", ".tpz",
    ".txz", ".xz",
];
const HEALTHCHECK_COMMAND: &str = "CMD";
const FROM_FLAG: &str = "from";
const USER_GROUP_SEPARATOR: char = ':';
const ENV_SEPARATOR: char = '=';
const ARG_DEFAULT_SEPARATOR: char = '=';
const SENSITIVE_NAMES: &[&str] = &[
    "ACCESS_TOKEN",
    "APPLICATION_KEY",
    "APP_SECRET",
    "AUTH_TOKEN",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "BITTREX_API_KEY",
    "BITTREX_API_SECRET",
    "CF_PASSWORD",
    "CF_USERNAME",
    "CIRCLE_TOKEN",
    "CI_DEPLOY_PASSWORD",
    "CI_DEPLOY_USER",
    "DOCKERHUB_PASSWORD",
    "DOCKER_EMAIL",
    "DOCKER_PASSWORD",
    "DOCKER_USERNAME",
    "FACEBOOK_ACCESS_TOKEN",
    "FACEBOOK_APP_ID",
    "FACEBOOK_APP_SECRET",
    "FIREBASE_API_TOKEN",
    "FIREBASE_TOKEN",
    "FOSSA_API_KEY",
    "GH_ENTERPRISE_TOKEN",
    "GH_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GITHUB_TOKEN",
    "HEROKU_API_KEY",
    "HEROKU_API_USER",
    "NPM_AUTH_TOKEN",
    "NPM_TOKEN",
    "OKTA_AUTHN_GROUPID",
    "OKTA_CLIENT_ORGURL",
    "OKTA_CLIENT_TOKEN",
    "OKTA_OAUTH2_CLIENTID",
    "OKTA_OAUTH2_CLIENTSECRET",
    "OPENAI_API_KEY",
    "OS_PASSWORD",
    "OS_USERNAME",
    "POSTGRES_PASSWORD",
    "SLACK_TOKEN",
    "STRIPE_API_KEY",
    "STRIPE_DEVICE_NAME",
    "TRAVIS_OS_NAME",
    "TRAVIS_SECURE_ENV_VARS",
    "TRAVIS_SUDO",
    "VAULT_CLIENT_KEY",
    "VAULT_TOKEN",
];
const SUSPICIOUS_FRAGMENTS: &[&str] = &[
    "api_key",
    "client_key",
    "password",
    "private",
    "secret",
    "token",
    "username",
];

pub(super) fn check(steps: &[Step], findings: &mut Findings) {
    let mut seen_from = false;
    let mut declared_arguments: Vec<String> = Vec::new();
    for step in steps {
        let instruction = &step.instruction;
        let opens_the_file =
            matches!(instruction.keyword, Keyword::From | Keyword::Arg) && !instruction.onbuild;
        if !seen_from && !opens_the_file {
            findings.report(DockerfileRule::Dl3061, instruction.line, String::new());
        }
        seen_from |= instruction.keyword == Keyword::From && !instruction.onbuild;
        if instruction.keyword == Keyword::Arg {
            declared_arguments.extend(argument_name(instruction));
        }
        sensitive_names(instruction, findings);
        if !instruction.onbuild {
            plain(instruction, &declared_arguments, findings);
        }
    }
}

fn plain(instruction: &Instruction, declared_arguments: &[String], findings: &mut Findings) {
    let line = instruction.line;
    match instruction.keyword {
        Keyword::Workdir => {
            let directory = unquoted(instruction.arguments.trim());
            if !(directory.starts_with(VARIABLE_PREFIX)
                || directory.starts_with(ROOT)
                || is_windows_absolute(directory))
            {
                findings.report(DockerfileRule::Dl3000, line, directory);
            }
        }
        Keyword::From => base_image(&BaseImage::parse(instruction), line, findings),
        Keyword::Add => add(instruction, findings),
        Keyword::Copy => copy(instruction, findings),
        Keyword::Cmd | Keyword::Entrypoint if instruction.exec_form().is_none() => {
            findings.report(DockerfileRule::Dl3025, line, instruction.arguments.trim());
        }
        Keyword::Healthcheck => {
            let words = instruction.arguments.trim();
            let command = words
                .split_once(char::is_whitespace)
                .filter(|(keyword, _)| keyword.eq_ignore_ascii_case(HEALTHCHECK_COMMAND));
            if let Some((_, command)) = command.filter(|(_, command)| !command.trim_start().starts_with('['))
            {
                findings.report(DockerfileRule::Dl3025, line, command.trim());
            }
        }
        Keyword::Maintainer => findings.report(DockerfileRule::Dl4000, line, instruction.arguments.trim()),
        Keyword::User => user(instruction, declared_arguments, findings),
        Keyword::Run
        | Keyword::Env
        | Keyword::Arg
        | Keyword::Cmd
        | Keyword::Entrypoint
        | Keyword::Shell
        | Keyword::Expose
        | Keyword::Onbuild
        | Keyword::Label
        | Keyword::Other => {}
    }
}

fn base_image(image: &BaseImage, line: usize, findings: &mut Findings) {
    if image.tag.as_deref() == Some(LATEST) && image.digest.is_none() {
        findings.report(DockerfileRule::Dl3007, line, image.name.clone());
    }
    let platform_is_variable = image.platform.as_deref().is_none_or(|platform| {
        PLATFORM_VARIABLES
            .iter()
            .any(|variable| platform.contains(variable))
    });
    if !platform_is_variable {
        findings.report(
            DockerfileRule::Dl3029,
            line,
            image.platform.clone().unwrap_or_default(),
        );
    }
}

pub(super) fn paths(instruction: &Instruction) -> Vec<String> {
    instruction.exec_form().unwrap_or_else(|| {
        instruction
            .arguments
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    })
}

fn add(instruction: &Instruction, findings: &mut Findings) {
    if instruction.heredoc.is_some() {
        return;
    }
    let paths = paths(instruction);
    let Some((_, sources)) = paths.split_last() else {
        return;
    };
    let copies_files = sources.iter().any(|source| {
        let source = unquoted(source);
        !(ARCHIVE_EXTENSIONS
            .iter()
            .any(|extension| source.ends_with(extension))
            || URL_PREFIXES.iter().any(|prefix| source.starts_with(prefix)))
    });
    if copies_files {
        findings.report(DockerfileRule::Dl3020, instruction.line, sources.join(" "));
    }
}

fn copy(instruction: &Instruction, findings: &mut Findings) {
    if instruction.heredoc.is_some() {
        return;
    }
    let paths = paths(instruction);
    let Some((target, sources)) = paths.split_last() else {
        return;
    };
    if sources.len() > 1 && !unquoted(target).ends_with('/') {
        findings.report(DockerfileRule::Dl3021, instruction.line, target.clone());
    }
    let copies_root = instruction.flag(FROM_FLAG).is_some()
        && unquoted(target) == ROOT
        && sources.iter().any(|source| unquoted(source) == ROOT);
    if copies_root {
        findings.report(
            DockerfileRule::Dl3067,
            instruction.line,
            instruction.flag(FROM_FLAG).unwrap_or_default(),
        );
    }
}

fn user(instruction: &Instruction, declared_arguments: &[String], findings: &mut Findings) {
    let user = instruction.arguments.trim();
    let uid = user.split(USER_GROUP_SEPARATOR).next().unwrap_or_default();
    let uses_argument = declared_arguments
        .iter()
        .any(|name| uid.contains(&format!("${{{name}}}")) || uid.contains(&format!("${name}")));
    if !uid.chars().all(|character| character.is_ascii_digit()) && !uses_argument {
        findings.report(DockerfileRule::Dl3066, instruction.line, user);
    }
}

fn argument_name(instruction: &Instruction) -> Option<String> {
    let text = instruction.arguments.trim();
    let name = text.split(ARG_DEFAULT_SEPARATOR).next()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn environment_names(instruction: &Instruction) -> Vec<String> {
    let text = instruction.arguments.trim();
    let first = text.split_whitespace().next().unwrap_or_default();
    if !first.contains(ENV_SEPARATOR) {
        return vec![first.to_owned()];
    }
    super::stages::environment_pairs(text)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

fn sensitive_names(instruction: &Instruction, findings: &mut Findings) {
    let names = match instruction.keyword {
        Keyword::Arg => argument_name(instruction).into_iter().collect(),
        Keyword::Env => environment_names(instruction),
        Keyword::From
        | Keyword::Run
        | Keyword::Copy
        | Keyword::Add
        | Keyword::User
        | Keyword::Workdir
        | Keyword::Cmd
        | Keyword::Entrypoint
        | Keyword::Healthcheck
        | Keyword::Shell
        | Keyword::Maintainer
        | Keyword::Expose
        | Keyword::Onbuild
        | Keyword::Label
        | Keyword::Other => return,
    };
    let sensitive = names.iter().find(|name| {
        let lowercase = name.to_lowercase();
        SENSITIVE_NAMES.contains(&name.to_uppercase().as_str())
            || SUSPICIOUS_FRAGMENTS
                .iter()
                .any(|fragment| lowercase.contains(fragment))
    });
    if let Some(name) = sensitive {
        findings.report(DockerfileRule::Dl3064, instruction.line, name.clone());
    }
}

pub(super) fn is_windows_absolute(path: &str) -> bool {
    let mut characters = path.chars();
    matches!((characters.next(), characters.next()), (Some(drive), Some(':')) if drive.is_alphabetic())
}
