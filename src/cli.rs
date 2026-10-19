use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::language::Stack;
use crate::limits::LimitOverride;
use crate::report::Format;
use crate::rules::RuleId;

#[derive(Debug, Parser)]
#[command(name = "kerbo", version, about = "Analizador de calidad multi-lenguaje")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Analiza los archivos y lista los problemas")]
    Check(CheckArgs),
    #[command(
        about = "Guarda los problemas actuales como línea de base: después sólo se reportan los nuevos"
    )]
    Baseline(ScanArgs),
    #[command(about = "Explica una regla: por qué existe y cómo se arregla")]
    Explain {
        #[arg(help = "La regla, por ejemplo cognitive-complexity")]
        rule: RuleId,
    },
    #[command(
        about = "Hook para agentes: revisa el archivo recién editado (lee el evento de Claude Code por stdin)"
    )]
    Hook(HookArgs),
    #[command(about = "Servidor MCP por stdio: herramientas kerbo_check y kerbo_explain para agentes")]
    Mcp,
    #[command(
        about = "Servidor LSP por stdio: los problemas en el editor mientras escribís (IntelliJ, VS Code, Neovim)"
    )]
    Lsp,
}

#[derive(Debug, Args)]
pub struct HookArgs {
    #[arg(
        long,
        value_name = "ARCHIVO",
        help = "Usa esta configuración en vez de buscar kerbo.toml"
    )]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    #[arg(default_value = ".", help = "Archivos o carpetas a analizar")]
    pub paths: Vec<PathBuf>,
    #[arg(
        long = "language",
        value_enum,
        help = "Analizar sólo estas pilas (se puede repetir)"
    )]
    pub languages: Vec<Stack>,
    #[arg(
        long = "max",
        value_name = "REGLA=VALOR",
        help = "Cambia el máximo de una regla en todos los lenguajes, también en los tests (se puede repetir)"
    )]
    pub overrides: Vec<LimitOverride>,
    #[arg(
        long,
        value_name = "ARCHIVO",
        help = "Usa esta configuración en vez de buscar kerbo.toml"
    )]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    #[command(flatten)]
    pub scan: ScanArgs,
    #[arg(long, value_enum, default_value_t = Format::Text, help = "Formato de salida")]
    pub format: Format,
    #[arg(
        long,
        value_name = "REFERENCIA",
        help = "Sólo los problemas en líneas cambiadas desde esta referencia de git"
    )]
    pub diff: Option<String>,
    #[arg(long, help = "Reporta también lo que está en la línea de base")]
    pub no_baseline: bool,
}
