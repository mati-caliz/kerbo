use super::{
    CaseShape, CognitiveShape, CountedLines, LineSpan, LogicalShape, ParameterShape, Profile, SpanBounds,
    TestFiles,
};

pub static GO: Profile = Profile {
    units: &["function_declaration", "method_declaration"],
    inline_functions: &["func_literal"],
    parameters: ParameterShape {
        checked_units: &["function_declaration", "method_declaration"],
        list_field: "parameters",
        single_field: None,
        counted: &["parameter_declaration", "variadic_parameter_declaration"],
        excluded_inner: &[],
        excluded_names: &[],
        exempting_annotation: None,
        names_per_declaration: true,
    },
    test_files: TestFiles {
        name_markers: &["_test.go"],
        directories: &[],
    },
    decisions: &["if_statement", "for_statement"],
    cases: CaseShape {
        kinds: &["expression_case", "type_case", "communication_case"],
        default_marker: None,
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
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
        ],
        flat: &["goto_statement"],
        labeled_jumps: &["break_statement", "continue_statement"],
    },
    comments: &["comment"],
    parenthesized: &["parenthesized_expression"],
    function_lines: LineSpan {
        bounds: SpanBounds::Interior,
        counted: CountedLines::All,
    },
    file_lines: LineSpan::ALL_LINES,
};
