mod analysis;
mod app;
mod baseline;
mod cache;
mod cli;
mod config;
mod daemon;
mod deploy;
mod diff;
mod duplicates;
mod fix;
mod guard;
mod i18n;
mod language;
mod limits;
mod lines;
mod lint;
mod macro_rules;
mod metrics;
mod osv;
mod profile;
mod report;
mod rules;
mod scan;
mod secrets;
mod shell;
mod suppressions;
mod syntax;
#[cfg(test)]
mod test_support;

use std::ffi::OsString;
use std::io::{self, Cursor};
use std::process::ExitCode;

use crate::app::{EXIT_FAILURE, Terminal, configured_lang, run};
use crate::cache::runtime_identity;
use crate::daemon::{Forwarded, Invocation, deliver, forward};
use crate::i18n::{Lang, LangPreference, Problem};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    let readable: Vec<String> = arguments
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let preference = LangPreference {
        flag: Lang::from_arguments(&readable),
        locale: Lang::from_locale(|variable| std::env::var(variable).ok()),
    };
    let Ok(directory) = std::env::current_dir() else {
        let lang = preference.resolve(None);
        eprintln!("kerbo: {}", lang.problem(&Problem::CurrentDirectory));
        return ExitCode::from(EXIT_FAILURE);
    };
    let help_lang = preference.resolve(configured_lang(None, &directory));
    let exact = arguments.iter().all(|argument| argument.to_str().is_some());
    let cli = cli::try_parse(help_lang, arguments).unwrap_or_else(|error| error.exit());
    let preference = LangPreference {
        flag: cli.lang,
        ..preference
    };
    let invocation = Invocation {
        command: &cli.command,
        arguments: &readable,
        directory: &directory,
        preference,
    };
    ExitCode::from(dispatch(&invocation, exact))
}

fn dispatch(invocation: &Invocation<'_>, exact_arguments: bool) -> u8 {
    let mut stdin = io::stdin().lock();
    let forwarded = daemon::endpoint_directory()
        .filter(|_| exact_arguments)
        .map(|endpoint| forward(&endpoint, runtime_identity(), invocation, &mut stdin));
    let replayed = match forwarded {
        Some(Forwarded::Done(finished)) => {
            let delivered = deliver(&finished, &mut io::stdout().lock(), &mut io::stderr().lock());
            return if delivered.is_ok() {
                finished.status
            } else {
                EXIT_FAILURE
            };
        }
        Some(Forwarded::Local { input }) => input,
        None => None,
    };
    replayed.map_or_else(
        || run_here(invocation, &mut stdin),
        |input| run_here(invocation, &mut Cursor::new(input)),
    )
}

fn run_here(invocation: &Invocation<'_>, input: &mut dyn io::Read) -> u8 {
    let mut terminal = Terminal {
        input,
        report: &mut io::stdout().lock(),
        log: &mut io::stderr().lock(),
    };
    run(
        invocation.command,
        invocation.preference,
        invocation.directory,
        &mut terminal,
    )
}
