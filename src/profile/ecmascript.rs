use super::{CaseShape, CognitiveShape, LineSpan, LogicalShape, ParameterShape, Profile, TestFiles};

const FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "generator_function_declaration",
    "generator_function",
    "arrow_function",
    "method_definition",
];

pub static ECMASCRIPT: Profile = Profile {
    units: FUNCTIONS,
    inline_functions: &[],
    parameters: ParameterShape {
        checked_units: FUNCTIONS,
        list_field: "parameters",
        single_field: Some("parameter"),
        counted: &[
            "required_parameter",
            "optional_parameter",
            "identifier",
            "assignment_pattern",
            "rest_pattern",
            "object_pattern",
            "array_pattern",
        ],
        excluded_inner: &[],
        excluded_names: &[],
        exempting_annotation: None,
        names_per_declaration: false,
    },
    test_files: TestFiles {
        name_markers: &[".test.", ".spec."],
        directories: &["test", "tests", "__tests__", "e2e"],
    },
    decisions: &[
        "if_statement",
        "ternary_expression",
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
    ],
    cases: CaseShape {
        kinds: &["switch_case"],
        default_marker: None,
    },
    logical: LogicalShape {
        kinds: &["binary_expression"],
        cognitive_operators: &["&&", "||"],
        cyclomatic_operators: &["&&", "||", "??"],
    },
    cognitive: CognitiveShape {
        if_kind: "if_statement",
        else_wrapper: Some("else_clause"),
        nesting: &[
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "catch_clause",
            "ternary_expression",
        ],
        flat: &[],
        labeled_jumps: &["break_statement", "continue_statement"],
    },
    comments: &["comment", "html_comment"],
    parenthesized: &["parenthesized_expression"],
    function_lines: LineSpan::CODE_LINES,
    file_lines: LineSpan::CODE_LINES,
};
