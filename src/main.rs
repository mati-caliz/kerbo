mod analysis;
mod app;
mod baseline;
mod cli;
mod config;
mod diff;
mod explain;
mod guard;
mod language;
mod limits;
mod lines;
mod metrics;
mod profile;
mod report;
mod rules;
mod scan;
mod syntax;
#[cfg(test)]
mod test_support;

use std::io;
use std::process::ExitCode;

use clap::Parser;

use crate::app::{EXIT_FAILURE, Terminal, run};
use crate::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Ok(directory) = std::env::current_dir() else {
        eprintln!("kerbo: no se pudo leer la carpeta actual");
        return ExitCode::from(EXIT_FAILURE);
    };
    let mut terminal = Terminal {
        input: &mut io::stdin().lock(),
        report: &mut io::stdout().lock(),
        log: &mut io::stderr().lock(),
    };
    ExitCode::from(run(&cli.command, &directory, &mut terminal))
}
