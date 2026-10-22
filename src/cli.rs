use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::i18n::{Lang, Problem, Text};
use crate::language::Stack;
use crate::limits::LimitOverride;
use crate::osv::{Ecosystem, OsvMode};
use crate::report::Format;
use crate::rules::RuleId;

const CHECK_COMMAND: &str = "check";
const BASELINE_COMMAND: &str = "baseline";
const EXPLAIN_COMMAND: &str = "explain";
const HOOK_COMMAND: &str = "hook";
const MCP_COMMAND: &str = "mcp";
const LSP_COMMAND: &str = "lsp";
const DAEMON_COMMAND: &str = "daemon";
const DAEMON_START_COMMAND: &str = "start";
const DAEMON_STOP_COMMAND: &str = "stop";
const DAEMON_STATUS_COMMAND: &str = "status";
const FIX_COMMAND: &str = "fix";
const FMT_COMMAND: &str = "fmt";
const CHECK_ARGUMENT: &str = "check";
const RULES_ARGUMENT: &str = "rules";
const DRY_RUN_ARGUMENT: &str = "dry_run";
const SINCE_ARGUMENT: &str = "since";
const LANG_ARGUMENT: &str = "lang";
const RULE_ARGUMENT: &str = "rule";
const CONFIG_ARGUMENT: &str = "config";
const PATHS_ARGUMENT: &str = "paths";
const LANGUAGES_ARGUMENT: &str = "languages";
const OVERRIDES_ARGUMENT: &str = "overrides";
const FORMAT_ARGUMENT: &str = "format";
const DIFF_ARGUMENT: &str = "diff";
const NO_BASELINE_ARGUMENT: &str = "no_baseline";
const OSV_ARGUMENT: &str = "osv";
const OSV_DOWNLOAD_COMMAND: &str = "osv-download";
const ECOSYSTEMS_ARGUMENT: &str = "ecosystems";
const ONLINE_MODE: &str = "online";

#[derive(Debug, Parser)]
#[command(name = "kerbo", version)]
pub struct Cli {
    #[arg(long, global = true, value_enum)]
    pub lang: Option<Lang>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Check(CheckArgs),
    Baseline(ScanArgs),
    Explain {
        #[arg(value_parser = localized_parser::<RuleId>(Lang::default()))]
        rule: RuleId,
    },
    Fix(FixArgs),
    Fmt(FmtArgs),
    Hook(HookArgs),
    Mcp,
    #[command(name = OSV_DOWNLOAD_COMMAND)]
    OsvDownload {
        #[arg(value_enum)]
        ecosystems: Vec<Ecosystem>,
    },
    Lsp,
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum DaemonAction {
    Start,
    Stop,
    Status,
    #[command(hide = true)]
    Run,
}

impl Command {
    pub fn config(&self) -> Option<&Path> {
        match self {
            Self::Check(arguments) => arguments.scan.config.as_deref(),
            Self::Baseline(arguments) => arguments.config.as_deref(),
            Self::Hook(arguments) => arguments.config.as_deref(),
            Self::Fix(arguments) => arguments.config.as_deref(),
            Self::Fmt(arguments) => arguments.config.as_deref(),
            Self::Explain { .. } | Self::Mcp | Self::Lsp | Self::Daemon { .. } | Self::OsvDownload { .. } => {
                None
            }
        }
    }
}

#[derive(Debug, Args)]
pub struct HookArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct FixArgs {
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,
    #[arg(long = "language", value_enum)]
    pub languages: Vec<Stack>,
    #[arg(long = "rule", value_parser = localized_parser::<RuleId>(Lang::default()))]
    pub rules: Vec<RuleId>,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long, visible_alias = "diff")]
    pub dry_run: bool,
    #[arg(long)]
    pub since: Option<String>,
}

#[derive(Debug, Args)]
pub struct FmtArgs {
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,
    #[arg(long = "language", value_enum)]
    pub languages: Vec<Stack>,
    #[arg(long = "max", value_parser = localized_parser::<LimitOverride>(Lang::default()))]
    pub overrides: Vec<LimitOverride>,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long, help = "Analiza todo de cero, sin leer ni escribir la caché")]
    pub no_cache: bool,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    #[command(flatten)]
    pub scan: ScanArgs,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    #[arg(long)]
    pub diff: Option<String>,
    #[arg(long)]
    pub no_baseline: bool,
    #[arg(long, value_enum, num_args = 0..=1, require_equals = true, default_missing_value = ONLINE_MODE)]
    pub osv: Option<OsvMode>,
}

pub fn try_parse<Argument>(
    lang: Lang,
    arguments: impl IntoIterator<Item = Argument>,
) -> Result<Cli, clap::Error>
where
    Argument: Into<OsString> + Clone,
{
    let matches = command(lang).try_get_matches_from(arguments)?;
    Cli::from_arg_matches(&matches)
}

pub fn command(lang: Lang) -> clap::Command {
    Cli::command()
        .about(lang.text(Text::AboutKerbo))
        .mut_arg(LANG_ARGUMENT, |arg| arg.help(lang.text(Text::HelpLang)))
        .mut_subcommand(CHECK_COMMAND, |check| {
            scan_command(check, lang)
                .about(lang.text(Text::AboutCheck))
                .mut_arg(FORMAT_ARGUMENT, |arg| arg.help(lang.text(Text::HelpFormat)))
                .mut_arg(DIFF_ARGUMENT, |arg| {
                    arg.help(lang.text(Text::HelpDiff))
                        .value_name(lang.text(Text::ValueReference))
                })
                .mut_arg(NO_BASELINE_ARGUMENT, |arg| {
                    arg.help(lang.text(Text::HelpNoBaseline))
                })
                .mut_arg(OSV_ARGUMENT, |arg| arg.help(lang.text(Text::HelpOsv)))
        })
        .mut_subcommand(OSV_DOWNLOAD_COMMAND, |download| {
            download
                .about(lang.text(Text::AboutOsvDownload))
                .mut_arg(ECOSYSTEMS_ARGUMENT, |arg| {
                    arg.help(lang.text(Text::HelpEcosystems))
                })
        })
        .mut_subcommand(BASELINE_COMMAND, |baseline| {
            scan_command(baseline, lang).about(lang.text(Text::AboutBaseline))
        })
        .mut_subcommand(EXPLAIN_COMMAND, |explain| {
            explain
                .about(lang.text(Text::AboutExplain))
                .mut_arg(RULE_ARGUMENT, |arg| {
                    arg.help(lang.text(Text::HelpRule))
                        .value_parser(localized_parser::<RuleId>(lang))
                })
        })
        .mut_subcommand(FIX_COMMAND, |fix| fix_command(fix, lang))
        .mut_subcommand(FMT_COMMAND, |fmt| fmt_command(fmt, lang))
        .mut_subcommand(HOOK_COMMAND, |hook| {
            config_argument(hook, lang).about(lang.text(Text::AboutHook))
        })
        .mut_subcommand(MCP_COMMAND, |mcp| mcp.about(lang.text(Text::AboutMcp)))
        .mut_subcommand(LSP_COMMAND, |lsp| lsp.about(lang.text(Text::AboutLsp)))
        .mut_subcommand(DAEMON_COMMAND, |daemon| daemon_command(daemon, lang))
}

fn daemon_command(command: clap::Command, lang: Lang) -> clap::Command {
    command
        .about(lang.text(Text::AboutDaemon))
        .mut_subcommand(DAEMON_START_COMMAND, |start| {
            start.about(lang.text(Text::AboutDaemonStart))
        })
        .mut_subcommand(DAEMON_STOP_COMMAND, |stop| {
            stop.about(lang.text(Text::AboutDaemonStop))
        })
        .mut_subcommand(DAEMON_STATUS_COMMAND, |status| {
            status.about(lang.text(Text::AboutDaemonStatus))
        })
}

fn scan_command(command: clap::Command, lang: Lang) -> clap::Command {
    config_argument(command, lang)
        .mut_arg(PATHS_ARGUMENT, |arg| arg.help(lang.text(Text::HelpPaths)))
        .mut_arg(LANGUAGES_ARGUMENT, |arg| arg.help(lang.text(Text::HelpLanguages)))
        .mut_arg(OVERRIDES_ARGUMENT, |arg| {
            arg.help(lang.text(Text::HelpMax))
                .value_name(lang.text(Text::ValueRuleLimit))
                .value_parser(localized_parser::<LimitOverride>(lang))
        })
}

fn fix_command(command: clap::Command, lang: Lang) -> clap::Command {
    config_argument(command, lang)
        .about(lang.text(Text::AboutFix))
        .mut_arg(PATHS_ARGUMENT, |arg| arg.help(lang.text(Text::HelpPaths)))
        .mut_arg(LANGUAGES_ARGUMENT, |arg| arg.help(lang.text(Text::HelpLanguages)))
        .mut_arg(RULES_ARGUMENT, |arg| {
            arg.help(lang.text(Text::HelpFixRule))
                .value_parser(localized_parser::<RuleId>(lang))
        })
        .mut_arg(DRY_RUN_ARGUMENT, |arg| arg.help(lang.text(Text::HelpDryRun)))
        .mut_arg(SINCE_ARGUMENT, |arg| {
            arg.help(lang.text(Text::HelpSince))
                .value_name(lang.text(Text::ValueReference))
        })
}

fn fmt_command(command: clap::Command, lang: Lang) -> clap::Command {
    config_argument(command, lang)
        .about(lang.text(Text::AboutFmt))
        .mut_arg(PATHS_ARGUMENT, |arg| arg.help(lang.text(Text::HelpPaths)))
        .mut_arg(CHECK_ARGUMENT, |arg| arg.help(lang.text(Text::HelpFmtCheck)))
}

fn config_argument(command: clap::Command, lang: Lang) -> clap::Command {
    command.mut_arg(CONFIG_ARGUMENT, |arg| {
        arg.help(lang.text(Text::HelpConfig))
            .value_name(lang.text(Text::ValueFile))
    })
}

fn localized_parser<Value>(
    lang: Lang,
) -> impl Fn(&str) -> Result<Value, String> + Clone + Send + Sync + 'static
where
    Value: FromStr<Err = Problem> + Clone + Send + Sync + 'static,
{
    move |text: &str| text.parse().map_err(|problem| lang.problem(&problem))
}

#[cfg(test)]
mod tests;
