mod ecmascript;
mod go;
mod java;
mod python;

pub use ecmascript::ECMASCRIPT;
pub use go::GO;
pub use java::JAVA;
pub use python::PYTHON;

pub type Kinds = &'static [&'static str];

#[derive(Debug)]
pub struct Profile {
    pub units: Kinds,
    pub inline_functions: Kinds,
    pub parameters: ParameterShape,
    pub test_files: TestFiles,
    pub decisions: Kinds,
    pub cases: CaseShape,
    pub logical: LogicalShape,
    pub cognitive: CognitiveShape,
    pub comments: Kinds,
    pub parenthesized: Kinds,
    pub function_lines: LineSpan,
    pub file_lines: LineSpan,
}

#[derive(Debug)]
pub struct ParameterShape {
    pub checked_units: Kinds,
    pub list_field: &'static str,
    pub single_field: Option<&'static str>,
    pub counted: Kinds,
    pub excluded_inner: Kinds,
    pub excluded_names: Kinds,
    pub exempting_annotation: Option<&'static str>,
    pub names_per_declaration: bool,
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
}

#[derive(Debug)]
pub struct LogicalShape {
    pub kinds: Kinds,
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
