use super::{
    CaseShape, CognitiveShape, CountedLines, LineSpan, LogicalShape, ParameterShape, Profile, SpanBounds,
    TestFiles,
};

pub static JAVA: Profile = Profile {
    units: &["method_declaration", "constructor_declaration"],
    inline_functions: &["lambda_expression"],
    parameters: ParameterShape {
        checked_units: &["method_declaration"],
        list_field: "parameters",
        single_field: None,
        counted: &["formal_parameter", "spread_parameter"],
        excluded_inner: &[],
        excluded_names: &[],
        exempting_annotation: Some("Override"),
        names_per_declaration: false,
    },
    test_files: TestFiles {
        name_markers: &[],
        directories: &["test"],
    },
    decisions: &[
        "if_statement",
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
        "ternary_expression",
    ],
    cases: CaseShape {
        kinds: &["switch_label"],
        default_marker: Some("default"),
    },
    logical: LogicalShape {
        kinds: &["binary_expression"],
        cognitive_operators: &["&&", "||"],
        cyclomatic_operators: &["&&", "||"],
    },
    cognitive: CognitiveShape {
        if_kind: "if_statement",
        else_wrapper: None,
        nesting: &[
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "switch_expression",
            "catch_clause",
            "ternary_expression",
        ],
        flat: &[],
        labeled_jumps: &["break_statement", "continue_statement"],
    },
    comments: &["line_comment", "block_comment"],
    parenthesized: &["parenthesized_expression"],
    function_lines: LineSpan {
        bounds: SpanBounds::FromBody,
        counted: CountedLines::Code,
    },
    file_lines: LineSpan::ALL_LINES,
};
