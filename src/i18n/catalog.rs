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
    AboutKerbo,
    AboutCheck,
    AboutBaseline,
    AboutExplain,
    AboutHook,
    AboutMcp,
    AboutLsp,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockUnit {
    Tokens,
    SyntaxNodes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    FileTooLong {
        lines: usize,
        limit: usize,
    },
    FunctionTooLong {
        function: String,
        lines: usize,
        limit: usize,
    },
    CyclomaticComplexity {
        function: String,
        value: usize,
        limit: usize,
    },
    CognitiveComplexity {
        function: String,
        value: usize,
        limit: usize,
    },
    TooManyParameters {
        function: String,
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
    Rule(RuleId),
    Violation {
        rule: RuleId,
        subject: String,
    },
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub files: usize,
    pub problems: usize,
    pub known: usize,
    pub milliseconds: u128,
    pub deviations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Summary(Summary),
    BaselineSaved { count: usize, path: String },
    HookHeader { count: usize },
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
