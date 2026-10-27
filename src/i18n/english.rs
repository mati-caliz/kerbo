use super::catalog::{
    BlockUnit, Catalog, Cheat, Explanation, Message, Notice, Problem, Summary, Text, counted, with_subject,
};
use crate::lint::{ImportRedundancy, LintKind};
use crate::rules::RuleId;

mod deploy;
mod dockerfile;
mod eslint;
mod explanations;
mod fixes;
mod go;
mod java;
mod python;
mod security;
mod shell;
mod taint;

const ANONYMOUS_FUNCTION: &str = "(anonymous)";

#[derive(Debug)]
pub struct English;

impl Catalog for English {
    fn text(&self, text: Text) -> &'static str {
        match text {
            Text::WhyLabel => "Why",
            Text::FixLabel => "How to fix it",
            Text::ExampleLabel => "Example",
            Text::McpInstructions => {
                "Use kerbo_check after editing code: with diff (for example HEAD) it looks only at what changed and catches cheats such as new suppressions or skipped tests. kerbo_fix applies the mechanical fixes at once (with dry_run it only returns the patch). kerbo_explain says how to fix each rule."
            }
            Text::McpCheckDescription => {
                "Analyzes files or folders and returns the prioritized problems, with a stable id and a fix hint."
            }
            Text::McpPathsDescription => "Files or folders; the project folder by default",
            Text::McpDiffDescription => "Git reference: only what changed since then, with anti-cheat checks",
            Text::McpExplainDescription => {
                "Explains a kerbo rule: why it exists, how to fix it and an example."
            }
            Text::McpFixDescription => {
                "Applies at once the mechanical fixes that do not overlap, analyzes the fixed files again and returns the patch."
            }
            Text::McpRulesDescription => "Only the fixes of these rules; all of them by default",
            Text::McpDryRunDescription => "Returns the patch without writing the files",
            Text::AboutFix => {
                "Applies at once every mechanical fix that does not overlap and analyzes the fixed files again"
            }
            Text::HelpFixRule => "Only the fixes of this rule (can be repeated)",
            Text::HelpDryRun => "Shows the patch without writing the files",
            Text::HelpUnsafeFixes => "Also applies the fixes that may change what the code does",
            Text::AboutFmt => "Formats the Go files exactly like gofmt",
            Text::HelpFmtCheck => {
                "Writes nothing: lists the files that would change and exits with 1 if there are any"
            }
            Text::HelpSince => "Fixes only the problems on lines changed since that git reference",
            Text::FixAllTitle => "fix everything mechanical in the file",
            Text::AboutKerbo => "Multi-language code quality analyzer",
            Text::AboutCheck => "Analyzes the files and lists the problems",
            Text::AboutBaseline => {
                "Saves the current problems as a baseline: from then on only new ones are reported"
            }
            Text::AboutExplain => "Explains a rule: why it exists and how to fix it",
            Text::AboutHook => {
                "Hook for agents: reviews the file that was just edited (reads the Claude Code event from stdin)"
            }
            Text::AboutMcp => {
                "MCP server over stdio: kerbo_check, kerbo_fix and kerbo_explain tools for agents"
            }
            Text::AboutLsp => {
                "LSP server over stdio: the problems in your editor while you type (IntelliJ, VS Code, Neovim)"
            }
            Text::AboutDaemon => {
                "One process per machine that check and the hook hand their work to, with the cache in memory"
            }
            Text::AboutDaemonStart => {
                "Starts the daemon in the background (it stops itself after 30 idle minutes)"
            }
            Text::AboutDaemonStop => "Stops the daemon",
            Text::AboutDaemonStatus => "Tells whether the daemon is running",
            Text::HelpRule => "The rule, for example cognitive-complexity",
            Text::HelpConfig => "Use this configuration instead of looking for kerbo.toml",
            Text::HelpPaths => "Files or folders to analyze",
            Text::HelpLanguages => "Analyze only these stacks (can be repeated)",
            Text::HelpMax => {
                "Changes the maximum of a rule in every language, tests included (can be repeated)"
            }
            Text::HelpFormat => "Output format",
            Text::HelpDiff => "Only the problems on lines changed since this git reference",
            Text::HelpNoBaseline => "Also report what is in the baseline",
            Text::HelpLang => "Language of the messages; if not given, the one in kerbo.toml or the system's",
            Text::ValueFile => "FILE",
            Text::ValueRuleLimit => "RULE=VALUE",
            Text::ValueReference => "REFERENCE",
            Text::AboutOsvDownload => {
                "Downloads the OSV database of each ecosystem to run `kerbo check --osv=offline` without network"
            }
            Text::HelpOsv => {
                "Looks up dependency vulnerabilities in OSV: online queries api.osv.dev, offline uses the downloaded database"
            }
            Text::HelpEcosystems => "Ecosystems to download; by default, every one kerbo reads",
        }
    }

    fn rule_description(&self, rule: RuleId) -> &'static str {
        match rule {
            RuleId::FileLength => "The file exceeds the maximum number of lines",
            RuleId::FunctionLength => "The function exceeds the maximum number of lines",
            RuleId::CyclomaticComplexity => "The function has too many possible paths",
            RuleId::CognitiveComplexity => "The function is hard to read: too many nested branches",
            RuleId::ParameterCount => "The function takes too many parameters",
            RuleId::SuppressionAdded => "The change silences a tool instead of fixing the cause",
            RuleId::TestDisabled => "The change skips or focuses a test, leaving others unrun",
            RuleId::TestRemoved => "The change deletes tests",
            RuleId::TypeEscape => "The change escapes the type system",
            RuleId::NestingDepth => "Too many nested blocks",
            RuleId::MagicNumber => "An unnamed number in the middle of the logic",
            RuleId::ShortName => "A name too short or ambiguous for its scope",
            RuleId::DuplicateCode => "A block of code copied and pasted somewhere else",
            RuleId::CommentedCode => "Code commented out instead of deleted",
            RuleId::UnusedImport => "An import that is not used",
            RuleId::RedundantImport => {
                "A redundant import: duplicated, from java.lang or from the same package"
            }
            RuleId::StarImport => "A wildcard import that brings in the whole package",
            RuleId::RedundantModifier => "A modifier that is already implied",
            RuleId::FinalParameter => "A parameter that is not final",
            RuleId::FinalClass => "A class nobody can extend that is not final",
            RuleId::Migration(rule) => deploy::migration(rule).description,
            RuleId::Dockerfile(rule) => dockerfile::dockerfile(rule).description,
            RuleId::Compose(rule) => deploy::compose(rule).description,
            RuleId::CommittedSecret => security::COMMITTED_SECRET_DESCRIPTION,
            RuleId::VulnerableDependency => security::VULNERABLE_DEPENDENCY_DESCRIPTION,
            RuleId::Python(rule) => python::python(rule).description,
            RuleId::Eslint(rule) => eslint::eslint(rule).description,
            RuleId::Shell(rule) => shell::shell(rule).description,
            RuleId::Go(rule) => go::go(rule).description,
            RuleId::Java(rule) => java::java(rule).description,
            RuleId::Taint(rule) => taint::taint(rule).description,
        }
    }

    fn explanation(&self, rule: RuleId) -> Explanation {
        explanations::explanation(rule)
    }

    fn message(&self, message: &Message) -> String {
        match message {
            Message::FileTooLong { lines, limit } => format!("The file has {lines} lines (maximum {limit})"),
            Message::FunctionTooLong {
                function,
                lines,
                limit,
            } => {
                let function = function.shown_as(ANONYMOUS_FUNCTION);
                format!("The function `{function}` has {lines} lines (maximum {limit})")
            }
            Message::CyclomaticComplexity {
                function,
                value,
                limit,
            } => {
                let function = function.shown_as(ANONYMOUS_FUNCTION);
                format!("The function `{function}` has cyclomatic complexity {value} (maximum {limit})")
            }
            Message::CognitiveComplexity {
                function,
                value,
                limit,
            } => {
                let function = function.shown_as(ANONYMOUS_FUNCTION);
                format!("The function `{function}` has cognitive complexity {value} (maximum {limit})")
            }
            Message::TooManyParameters {
                function,
                count,
                limit,
            } => {
                let function = function.shown_as(ANONYMOUS_FUNCTION);
                format!("The function `{function}` takes {count} parameters (maximum {limit})")
            }
            Message::NestedTooDeep { depth, limit } => {
                format!("This block is {depth} levels deep (maximum {limit})")
            }
            Message::UnnamedNumber { number } => format!("The number `{number}` has no name"),
            Message::ShortName { name } => format!("`{name}` is a name too short or ambiguous"),
            Message::RepeatedFunction { original_line } => {
                format!("This function repeats the one on line {original_line}")
            }
            Message::RepeatedBranch { original_line } => {
                format!("This branch repeats the one on line {original_line}")
            }
            Message::DuplicatedBlock {
                size,
                unit,
                copies,
                limit,
            } => format!(
                "This block of {size} {} is repeated in {copies} (maximum {limit})",
                block_unit(*unit)
            ),
            Message::Lint { kind, subject } => self.lint(*kind, subject),
            Message::Cheat(cheat) => self.cheat(cheat),
            Message::CommittedSecret(leak) => security::secret(leak),
            Message::VulnerableDependency(advisory) => security::advisory(advisory),
            Message::Rule(rule) => self.rule_description(*rule).to_owned(),
            Message::Violation { rule, subject } => with_subject(self.rule_description(*rule), subject),
        }
    }

    fn lint(&self, kind: LintKind, subject: &str) -> String {
        match kind {
            LintKind::UnusedImport => format!("The import `{subject}` is not used"),
            LintKind::RedundantImport(ImportRedundancy::JavaLang) => {
                format!("`{subject}` is in java.lang, which is imported implicitly")
            }
            LintKind::RedundantImport(ImportRedundancy::SamePackage) => {
                format!("`{subject}` is in the same package: it needs no import")
            }
            LintKind::RedundantImport(ImportRedundancy::Duplicate { first_line }) => {
                format!("`{subject}` is already imported on line {first_line}")
            }
            LintKind::StarImport => format!("`{subject}` imports the whole package"),
            LintKind::RedundantModifier => {
                format!("The `{subject}` modifier is redundant: it is already implied")
            }
            LintKind::Eslint(rule) => with_subject(eslint::eslint(rule).description, subject),
            LintKind::FinalParameter => format!("The parameter `{subject}` should be final"),
            LintKind::FinalClass => {
                format!("The class `{subject}` should be final: it cannot be extended from outside")
            }
            LintKind::Python(rule) => with_subject(python::python(rule).description, subject),
            LintKind::Go(rule) => with_subject(go::go(rule).description, subject),
            LintKind::Java(rule) => with_subject(java::java(rule).description, subject),
            LintKind::Taint(rule) => with_subject(taint::taint(rule).description, subject),
        }
    }

    fn cheat(&self, cheat: &Cheat) -> String {
        match cheat {
            Cheat::TestsRemoved {
                removed,
                before,
                after,
                net,
            } => format!(
                "{} deleted from this file ({before} before, {after} now); the whole change has {} fewer",
                counted(*removed, "test was", "tests were"),
                counted(*net, "test", "tests")
            ),
            Cheat::EmptyTest { name } => {
                format!("The test `{name}` is empty: it passes without testing anything")
            }
            Cheat::TestSkipped { marker } => {
                format!("`{marker}` leaves tests unrun: fix the test instead of skipping it")
            }
            Cheat::TypeEscaped { marker } => {
                format!("`{marker}` escapes the type system: give the value a real type")
            }
            Cheat::SuppressionAdded { marker } => {
                format!(
                    "`{marker}` was added: fix the cause, or record the exception in kerbo.toml with its reason"
                )
            }
            Cheat::ThresholdLoosened { old, new } => {
                format!(
                    "A threshold was loosened: `{old}` became `{new}`. Fix the code instead of lowering the bar"
                )
            }
        }
    }

    fn problem(&self, problem: &Problem) -> String {
        match problem {
            Problem::CurrentDirectory => "could not read the current folder".to_owned(),
            Problem::WriteReport(detail) => format!("could not write the report: {detail}"),
            Problem::Unreadable { path, detail } => format!("{path}: {detail}"),
            Problem::Unparsable { path } => format!("{path}: tree-sitter returned no tree"),
            Problem::MissingReason { path } => {
                format!("{path}: every adjustment needs a `reason` saying why it is loosened")
            }
            Problem::MaxWithoutRule { path } => {
                format!("{path}: an exception with `max` must name the `rule`")
            }
            Problem::InvalidPattern {
                path,
                pattern,
                detail,
            } => format!("{path}: pattern `{pattern}`: {detail}"),
            Problem::UnknownBaselineVersion { path, version } => format!("{path}: unknown version {version}"),
            Problem::GitUnavailable(detail) => format!("could not run git: {detail}"),
            Problem::GoTypesUnavailable { module, detail } => format!(
                "{module}: errcheck, qf1008 and qf1012 need Go's types and they could not be obtained (it needs `go`; otherwise turn them off in kerbo.toml): {detail}"
            ),
            Problem::GitFailed { command, detail } => format!("git {command}: {detail}"),
            Problem::UnknownRule(rule) => format!("there is no rule `{rule}`"),
            Problem::ExpectedRuleLimit(text) => format!("expected rule=value and got `{text}`"),
            Problem::NotANonNegativeInteger(text) => format!("`{text}` is not a non-negative integer"),
            Problem::ReadEvent(detail) => format!("could not read the event: {detail}"),
            Problem::DaemonUnavailable(detail) => format!("could not start the daemon: {detail}"),
            Problem::DaemonWithoutDirectory => {
                "there is no cache directory for the daemon (set KERBO_CACHE_DIR)".to_owned()
            }
            Problem::InvalidEvent(detail) => format!("the event is not valid JSON: {detail}"),
            Problem::ReadRequest(detail) => format!("could not read the request: {detail}"),
            Problem::InvalidRequest => "the request is not valid JSON".to_owned(),
            Problem::UnknownMethod(method) => format!("unknown method: {method}"),
            Problem::UnknownTool(tool) => format!("unknown tool: {tool}"),
            Problem::InvalidArguments(detail) => format!("invalid arguments: {detail}"),
            Problem::ReadEditor(detail) => format!("could not read from the editor: {detail}"),
            Problem::MissingContentLength => "the Content-Length header is missing".to_owned(),
            Problem::IncompleteMessage(detail) => format!("incomplete message: {detail}"),
            Problem::InvalidMessage(detail) => format!("the message is not valid JSON: {detail}"),
            Problem::Osv(problem) => security::problem(problem),
        }
    }

    fn notice(&self, notice: &Notice) -> String {
        match notice {
            Notice::Summary(summary) => summarize(summary),
            Notice::Suppressed {
                path,
                line,
                rule,
                reason,
            } => fixes::suppressed(path, *line, *rule, reason),
            Notice::Fix(notice) => fixes::notice(notice),
            Notice::Format(notice) => fixes::format_notice(notice),
            Notice::DaemonStarted { pid, port } => format!("daemon started (process {pid}, port {port})"),
            Notice::DaemonAlreadyRunning { pid, port } => {
                format!("the daemon was already running (process {pid}, port {port})")
            }
            Notice::DaemonRunning { pid, port } => {
                format!("the daemon is running (process {pid}, port {port})")
            }
            Notice::DaemonStopped => "daemon stopped".to_owned(),
            Notice::DaemonNotRunning => "the daemon is not running".to_owned(),
            Notice::OsvDatabaseSaved {
                ecosystem,
                advisories,
                path,
            } => security::database_saved(ecosystem, *advisories, path),
            Notice::BaselineSaved { count, path } => {
                format!("{} saved in {path}", counted(*count, "problem", "problems"))
            }
            Notice::HookHeader { count } => {
                format!(
                    "{} in what you just edited:",
                    counted(*count, "problem", "problems")
                )
            }
        }
    }
}

fn summarize(summary: &Summary) -> String {
    let known = if summary.known > 0 {
        format!(" (and {} in the baseline)", summary.known)
    } else {
        String::new()
    };
    let deviations = if summary.deviations > 0 {
        let deviations = counted(summary.deviations, "deviation", "deviations");
        format!(" · {deviations} from the preset in kerbo.toml")
    } else {
        String::new()
    };
    format!(
        "{}, {}{known}, {} ms{deviations}{}",
        counted(summary.files, "file", "files"),
        counted(summary.problems, "problem", "problems"),
        summary.milliseconds,
        fixes::suppressed_suffix(summary.suppressed)
    )
}

const fn block_unit(unit: BlockUnit) -> &'static str {
    match unit {
        BlockUnit::Tokens => "tokens",
        BlockUnit::SyntaxNodes => "nodes",
    }
}
