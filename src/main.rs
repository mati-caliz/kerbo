mod analysis;
mod app;
mod baseline;
mod cache;
mod cli;
mod config;
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
mod suppressions;
mod syntax;
#[cfg(test)]
mod test_support;

use std::ffi::OsString;
use std::io;
use std::process::ExitCode;

use crate::app::{EXIT_FAILURE, Terminal, configured_lang, run};
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
    let cli = cli::try_parse(help_lang, arguments).unwrap_or_else(|error| error.exit());
    let mut terminal = Terminal {
        input: &mut io::stdin().lock(),
        report: &mut io::stdout().lock(),
        log: &mut io::stderr().lock(),
    };
    let preference = LangPreference {
        flag: cli.lang,
        ..preference
    };
    ExitCode::from(run(&cli.command, preference, &directory, &mut terminal))
}
