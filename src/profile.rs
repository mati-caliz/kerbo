mod ecmascript;
mod go;
mod java;
mod python;
mod rust;

pub use ecmascript::ECMASCRIPT;
pub use go::GO;
pub use java::JAVA;
pub use python::PYTHON;
pub use rust::RUST;

pub const NO_PARTIAL_UNITS: PartialUnits = PartialUnits {
    complexity_only: &[],
    signature_only: &[],
};

pub type Kinds = &'static [&'static str];

#[derive(Debug)]
pub struct Profile {
    pub metrics: MetricsStyle,
    pub units: Kinds,
    pub inline_functions: Kinds,
    pub partial_units: PartialUnits,
    pub leading_attachments: Kinds,
    pub parameters: ParameterShape,
    pub test_files: TestFiles,
    pub guard: GuardShape,
    pub decisions: Kinds,
    pub decision_fields: &'static [(&'static str, &'static str)],
    pub decision_parents: &'static [(&'static str, &'static str)],
    pub nested_units_add_to_cyclomatic: bool,
    pub cases: CaseShape,
    pub logical: LogicalShape,
    pub cognitive: CognitiveShape,
    pub nesting: NestingStyle,
    pub magic_numbers: MagicNumberStyle,
    pub short_names: ShortNameStyle,
    pub duplicates: DuplicateStyle,
    pub commented_code: CommentedCodeStyle,
    pub lints: LintStyle,
    pub comments: Kinds,
    pub literals: Kinds,
    pub function_length_skips_iife: bool,
    pub parenthesized: Kinds,
    pub function_lines: LineSpan,
    pub file_lines: LineSpan,
}

#[derive(Debug)]
pub struct PartialUnits {
    pub complexity_only: &'static [(&'static str, Option<&'static str>)],
    pub signature_only: Kinds,
}

#[derive(Debug)]
pub struct ParameterShape {
    pub checked_units: Kinds,
    pub list_field: &'static str,
    pub single_field: Option<&'static str>,
    pub counted: Kinds,
    pub excluded_inner: Kinds,
    pub static_decorator: Option<&'static str>,
    pub method_container: Option<&'static str>,
    pub excludes_dummy_names: bool,
    pub exempting_annotations: Kinds,
    pub names_per_declaration: bool,
}

#[derive(Debug)]
pub struct GuardShape {
    pub suppression_comments: Kinds,
    pub annotation_kinds: Kinds,
    pub suppression_annotations: Kinds,
    pub test_annotations: Kinds,
    pub disabled_annotations: Kinds,
    pub call_kind: &'static str,
    pub test_calls: Kinds,
    pub disabled_calls: Kinds,
    pub test_function: Option<(&'static str, &'static str)>,
    pub escape_types: &'static [(&'static str, &'static str)],
    pub double_cast: Option<(&'static str, &'static str)>,
}

#[derive(Debug)]
pub struct TestFiles {
    pub name_markers: Kinds,
    pub directories: Kinds,
}

#[derive(Debug)]
pub struct CaseShape {
    pub kinds: Kinds,
    pub default_marker: Option<&'static str>,
    pub wildcard_pattern: Option<&'static str>,
}

#[derive(Debug)]
pub struct LogicalShape {
    pub kinds: Kinds,
    pub sequence_operators: Kinds,
    pub cognitive_operators: Kinds,
    pub cyclomatic_operators: Kinds,
}

#[derive(Debug)]
pub struct CognitiveShape {
    pub if_kind: &'static str,
    pub else_wrapper: Option<&'static str>,
    pub nesting: Kinds,
    pub flat: Kinds,
    pub labeled_jumps: Kinds,
    pub short_circuit: Option<ShortCircuit>,
    pub leaking_branch_parent: Option<&'static str>,
    pub pmd: Option<PmdShape>,
    pub skipped_units: Kinds,
    pub counts_recursion: bool,
}

#[derive(Debug)]
pub struct PmdShape {
    pub structural: Kinds,
    pub nesting_only: Kinds,
    pub blocks: Kinds,
    pub negation: (&'static str, &'static str),
    pub invocation: &'static str,
    pub this: &'static str,
}

#[derive(Debug)]
pub struct ShortCircuit {
    pub container: &'static str,
    pub conditional: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricsStyle {
    Shared,
    Clippy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortNameStyle {
    Unmeasured,
    Eslint,
    Checkstyle,
    Varnamelen,
    Ruff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintStyle {
    Unmeasured,
    Checkstyle,
    Ruff,
    Eslint,
    GolangciLint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentedCodeStyle {
    Unmeasured,
    Eradicate,
    Gocritic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateStyle {
    Unmeasured,
    CpdJava,
    CpdPython,
    Dupl,
    Sonar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagicNumberStyle {
    Unmeasured,
    Eslint,
    Checkstyle,
    Mnd,
    Ruff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestingStyle {
    Unmeasured,
    Blocks,
    ControlFlow,
    Clippy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanBounds {
    Whole,
    FromBody,
    Interior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountedLines {
    All,
    Code,
    Tokens,
}

#[derive(Debug, Clone, Copy)]
pub struct LineSpan {
    pub bounds: SpanBounds,
    pub counted: CountedLines,
}

impl LineSpan {
    pub const ALL_LINES: Self = Self {
        bounds: SpanBounds::Whole,
        counted: CountedLines::All,
    };
    pub const CODE_LINES: Self = Self {
        bounds: SpanBounds::Whole,
        counted: CountedLines::Code,
    };
}
