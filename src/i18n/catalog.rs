use crate::lint::LintKind;
use crate::rules::RuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Explanation {
    pub fix: &'static str,
    pub why: &'static str,
    pub example: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleText {
    pub description: &'static str,
    pub explanation: Explanation,
}

impl RuleText {
    pub const fn new(
        description: &'static str,
        fix: &'static str,
        why: &'static str,
        example: &'static str,
    ) -> Self {
        Self {
            description,
            explanation: Explanation { fix, why, example },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    WhyLabel,
    FixLabel,
    ExampleLabel,
    McpInstructions,
    McpCheckDescription,
    McpPathsDescription,
    McpDiffDescription,
    McpExplainDescription,
    McpFixDescription,
    McpRulesDescription,
    McpDryRunDescription,
    AboutKerbo,
    AboutCheck,
    AboutBaseline,
    AboutExplain,
    AboutHook,
    AboutMcp,
    AboutLsp,
    AboutFix,
    HelpFixRule,
    HelpDryRun,
    FixAllTitle,
    HelpRule,
    HelpConfig,
    HelpPaths,
    HelpLanguages,
    HelpMax,
    HelpFormat,
    HelpDiff,
    HelpNoBaseline,
    HelpLang,
    ValueFile,
    ValueRuleLimit,
    ValueReference,
    AboutOsvDownload,
    HelpOsv,
    HelpEcosystems,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockUnit {
    Tokens,
    SyntaxNodes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionName {
    Named(String),
    Anonymous,
}

impl FunctionName {
    pub fn shown_as<'name>(&'name self, anonymous: &'name str) -> &'name str {
        match self {
            Self::Named(name) => name,
            Self::Anonymous => anonymous,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    FileTooLong {
        lines: usize,
        limit: usize,
    },
    FunctionTooLong {
        function: FunctionName,
        lines: usize,
        limit: usize,
    },
    CyclomaticComplexity {
        function: FunctionName,
        value: usize,
        limit: usize,
    },
    CognitiveComplexity {
        function: FunctionName,
        value: usize,
        limit: usize,
    },
    TooManyParameters {
        function: FunctionName,
        count: usize,
        limit: usize,
    },
    NestedTooDeep {
        depth: usize,
        limit: usize,
    },
    UnnamedNumber {
        number: String,
    },
    ShortName {
        name: String,
    },
    RepeatedFunction {
        original_line: usize,
    },
    RepeatedBranch {
        original_line: usize,
    },
    DuplicatedBlock {
        size: usize,
        unit: BlockUnit,
        copies: String,
        limit: usize,
    },
    Lint {
        kind: LintKind,
        subject: String,
    },
    Cheat(Cheat),
    CommittedSecret(SecretLeak),
    VulnerableDependency(Box<Advisory>),
    Rule(RuleId),
    Violation {
        rule: RuleId,
        subject: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretLeak {
    pub kind: String,
    pub preview: String,
    pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advisory {
    pub package: String,
    pub version: String,
    pub id: String,
    pub aliases: Vec<String>,
    pub summary: String,
    pub fixed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsvProblem {
    Unavailable(String),
    InvalidResponse(String),
    DatabaseMissing(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cheat {
    TestsRemoved {
        removed: usize,
        before: usize,
        after: usize,
        net: usize,
    },
    EmptyTest {
        name: String,
    },
    TestSkipped {
        marker: String,
    },
    TypeEscaped {
        marker: String,
    },
    SuppressionAdded {
        marker: String,
    },
    ThresholdLoosened {
        old: String,
        new: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    CurrentDirectory,
    WriteReport(String),
    Unreadable {
        path: String,
        detail: String,
    },
    Unparsable {
        path: String,
    },
    MissingReason {
        path: String,
    },
    MaxWithoutRule {
        path: String,
    },
    InvalidPattern {
        path: String,
        pattern: String,
        detail: String,
    },
    UnknownBaselineVersion {
        path: String,
        version: u32,
    },
    GitUnavailable(String),
    GitFailed {
        command: String,
        detail: String,
    },
    UnknownRule(String),
    ExpectedRuleLimit(String),
    NotANonNegativeInteger(String),
    ReadEvent(String),
    InvalidEvent(String),
    ReadRequest(String),
    InvalidRequest,
    UnknownMethod(String),
    UnknownTool(String),
    InvalidArguments(String),
    ReadEditor(String),
    MissingContentLength,
    IncompleteMessage(String),
    InvalidMessage(String),
    Osv(OsvProblem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub files: usize,
    pub problems: usize,
    pub known: usize,
    pub milliseconds: u128,
    pub deviations: usize,
    pub suppressed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Summary(Summary),
    BaselineSaved {
        count: usize,
        path: String,
    },
    HookHeader {
        count: usize,
    },
    Suppressed {
        path: String,
        line: usize,
        rule: RuleId,
        reason: String,
    },
    Fix(FixNotice),
    OsvDatabaseSaved {
        ecosystem: String,
        advisories: usize,
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixNotice {
    Applied { count: usize, files: usize },
    Previewed { count: usize, files: usize },
    Withheld { path: String, introduced: usize },
    Left { count: usize },
}

pub(super) const SINGLE: usize = 1;

pub fn counted(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == SINGLE { singular } else { plural };
    format!("{count} {noun}")
}

pub fn with_subject(description: &str, subject: &str) -> String {
    if subject.is_empty() {
        description.to_owned()
    } else {
        format!("{description}: `{subject}`")
    }
}

pub trait Catalog: Sync {
    fn text(&self, text: Text) -> &'static str;
    fn rule_description(&self, rule: RuleId) -> &'static str;
    fn explanation(&self, rule: RuleId) -> Explanation;
    fn message(&self, message: &Message) -> String;
    fn cheat(&self, cheat: &Cheat) -> String;
    fn lint(&self, kind: LintKind, subject: &str) -> String;
    fn problem(&self, problem: &Problem) -> String;
    fn notice(&self, notice: &Notice) -> String;
}
