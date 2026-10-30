use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::{Found, Posix};
use crate::rules::ShellRule;
use crate::shell::commands::{Command, is_flag};
use crate::shell::context::Context;
use crate::shell::words::{literal, literal_or, oversimplify};

mod set_options;

const ECHO: &str = "echo";
const EXEC: &str = "exec";
const LET: &str = "let";
const SET: &str = "set";
const LOCAL: &str = "local";
const SOURCE: &str = "source";
const TRAP: &str = "trap";
const PRINTF: &str = "printf";
const READ: &str = "read";
const FLAG_PREFIX: &str = "-";
const PATH_SEPARATOR: char = '/';
const LONG_FLAG_PREFIX: &str = "--";
const END_OF_FLAGS: &str = "--";
const LONG_FLAG_VALUE: char = '=';
const QUOTED_FORMAT: &str = "%q";
const DASH_ECHO_FLAG: &str = "-n";
const SIGNAL_PREFIX: &str = "SIG";
const BASH_TRAPS: &[&str] = &["ERR", "DEBUG", "RETURN"];
const UNSUPPORTED_COMMANDS: &[&str] = &[
    "let",
    "caller",
    "builtin",
    "complete",
    "compgen",
    "declare",
    "dirs",
    "disown",
    "enable",
    "mapfile",
    "readarray",
    "pushd",
    "popd",
    "shopt",
    "suspend",
    "typeset",
];
const ECHO_FLAGS: &str = "^-[eEsn]+$";
const BUSYBOX_ECHO_FLAGS: &str = "^-[en]+$";

static ECHO_FLAGS_REGEX: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(ECHO_FLAGS).ok());
static BUSYBOX_ECHO_FLAGS_REGEX: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(BUSYBOX_ECHO_FLAGS).ok());

fn matches(regex: &LazyLock<Option<Regex>>, text: &str) -> bool {
    regex.as_ref().is_some_and(|regex| regex.is_match(text))
}

// Las opciones que POSIX sí define para cada comando: las demás son de bash. `None` es que en ese
// dialecto ShellCheck no las revisa.
struct AllowedFlags {
    command: &'static str,
    sh: Option<&'static [&'static str]>,
    dash: Option<&'static [&'static str]>,
    busybox: Option<&'static [&'static str]>,
}

const fn everywhere(command: &'static str, flags: &'static [&'static str]) -> AllowedFlags {
    AllowedFlags {
        command,
        sh: Some(flags),
        dash: Some(flags),
        busybox: Some(flags),
    }
}

const ALLOWED_FLAGS: &[AllowedFlags] = &[
    everywhere("cd", &["L", "P"]),
    everywhere("exec", &[]),
    everywhere("export", &["p"]),
    AllowedFlags {
        command: "hash",
        sh: Some(&["r"]),
        dash: Some(&["r", "v"]),
        busybox: Some(&["r", "v"]),
    },
    everywhere("jobs", &["l", "p"]),
    everywhere("printf", &[]),
    AllowedFlags {
        command: "read",
        sh: Some(&["r"]),
        dash: Some(&["r", "p"]),
        busybox: Some(&["r", "p"]),
    },
    everywhere("readonly", &["p"]),
    everywhere("trap", &[]),
    AllowedFlags {
        command: "type",
        sh: Some(&[]),
        dash: Some(&[]),
        busybox: Some(&["p"]),
    },
    AllowedFlags {
        command: "ulimit",
        sh: Some(&["f"]),
        dash: None,
        busybox: None,
    },
    everywhere("umask", &["S"]),
    everywhere("unset", &["f", "v"]),
    everywhere("wait", &[]),
];

fn allowed_flags(name: &str, shell: Posix) -> Option<&'static [&'static str]> {
    let allowed = ALLOWED_FLAGS.iter().find(|allowed| allowed.command == name)?;
    match shell {
        Posix::Sh => allowed.sh,
        Posix::Dash => allowed.dash,
        Posix::Busybox => allowed.busybox,
    }
}

pub fn check(node: Node<'_>, context: &Context<'_, '_>, found: &mut Found<'_>) {
    let Some(command) = Command::of(node) else {
        return;
    };
    let source = context.source;
    if command.first_word().is_none() {
        return;
    }
    let name = command.name(source).unwrap_or_default();
    let raw_name = command.literal_name(source).unwrap_or_default();
    let argument = command.arguments().first().copied();
    let argument_text = argument
        .map(|word| oversimplify(word, source))
        .unwrap_or_default();
    if let Some(argument) =
        argument.filter(|_| is_command(&name, ECHO) && matches(&ECHO_FLAGS_REGEX, &argument_text))
    {
        check_echo_flags(argument, &argument_text, found);
        return;
    }
    if let Some(argument) = argument.filter(|_| raw_name == EXEC && argument_text.starts_with(FLAG_PREFIX)) {
        found.report(ShellRule::Sc3038, argument, &argument_text);
        return;
    }
    if is_command(&name, LET) {
        found.report(ShellRule::Sc3039, node, LET);
        return;
    }
    if is_command(&name, SET) {
        if !found.shell().is_dash() {
            set_options::check(command.arguments(), source, found);
        }
        return;
    }
    check_command(&command, &name, source, found);
}

// `isCommand` de ShellCheck: el nombre, o una ruta que termina en él (`/bin/echo`).
fn is_command(name: &str, expected: &str) -> bool {
    name == expected
        || name
            .strip_suffix(expected)
            .is_some_and(|directory| directory.ends_with(PATH_SEPARATOR))
}

fn check_echo_flags(argument: Node<'_>, flags: &str, found: &mut Found<'_>) {
    match found.shell() {
        Posix::Busybox if !matches(&BUSYBOX_ECHO_FLAGS_REGEX, flags) => {
            found.report(ShellRule::Sc3036, argument, flags);
        }
        Posix::Dash if flags != DASH_ECHO_FLAG => found.report(ShellRule::Sc3036, argument, flags),
        Posix::Sh => found.report(ShellRule::Sc3037, argument, flags),
        Posix::Busybox | Posix::Dash => {}
    }
}

fn check_command(command: &Command<'_>, name: &str, source: &[u8], found: &mut Found<'_>) {
    let node = command.node;
    if name == LOCAL && !found.shell().is_dash() {
        found.report(ShellRule::Sc3043, node, LOCAL);
    }
    if UNSUPPORTED_COMMANDS.contains(&name) {
        found.report(ShellRule::Sc3044, node, name);
    }
    if let Some(allowed) = allowed_flags(name, found.shell())
        && let Some((word, flag)) = leading_flags(command.arguments(), source)
            .into_iter()
            .find(|(_, flag)| !allowed.contains(&flag.as_str()))
    {
        found.report(ShellRule::Sc3045, word, &format!("{name} -{flag}"));
    }
    if name == SOURCE && !found.shell().is_busybox() {
        found.report(ShellRule::Sc3046, node, SOURCE);
    }
    if name == TRAP {
        for signal in command.arguments().iter().skip(1) {
            check_signal(*signal, source, found);
        }
    }
    if name == PRINTF
        && let Some(format) = command.arguments().first()
        && literal_or(*format, source, "").contains(QUOTED_FORMAT)
    {
        found.report(ShellRule::Sc3050, *format, QUOTED_FORMAT);
    }
    if name == READ
        && command
            .arguments()
            .iter()
            .all(|argument| is_flag(*argument, source))
        && let Some(first) = command.first_word()
    {
        found.report(ShellRule::Sc3061, first, READ);
    }
}

fn check_signal(signal: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    let Some(name) = literal(signal, source) else {
        return;
    };
    let upper = name.to_uppercase();
    if BASH_TRAPS.contains(&upper.as_str()) {
        found.report(ShellRule::Sc3047, signal, &name);
    }
    if !found.shell().is_busybox() && upper.starts_with(SIGNAL_PREFIX) {
        found.report(ShellRule::Sc3048, signal, &name);
    }
    if !found.shell().is_dash() && upper != name {
        found.report(ShellRule::Sc3049, signal, &name);
    }
}

// `getLeadingFlags` de ShellCheck: las opciones hasta `--` o el primer argumento que no empieza con
// `-`, una por letra, y las largas por su nombre.
fn leading_flags<'tree>(arguments: &[Node<'tree>], source: &[u8]) -> Vec<(Node<'tree>, String)> {
    let mut flags = Vec::new();
    for argument in arguments {
        let text = oversimplify(*argument, source);
        if text == END_OF_FLAGS || !text.starts_with(FLAG_PREFIX) {
            break;
        }
        if let Some(long) = text.strip_prefix(LONG_FLAG_PREFIX) {
            let name: String = long
                .chars()
                .take_while(|letter| *letter != LONG_FLAG_VALUE)
                .collect();
            flags.push((*argument, name));
        } else if let Some(short) = text.strip_prefix(FLAG_PREFIX) {
            flags.extend(short.chars().map(|letter| (*argument, letter.to_string())));
        }
    }
    flags.retain(|(_, flag)| !flag.is_empty());
    flags
}
