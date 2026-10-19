use super::{CaseShape, CognitiveShape, LineSpan, LogicalShape, ParameterShape, Profile, TestFiles};

pub static PYTHON: Profile = Profile {
    units: &["function_definition"],
    inline_functions: &["lambda"],
    parameters: ParameterShape {
        checked_units: &["function_definition"],
        list_field: "parameters",
        single_field: None,
        counted: &[
            "identifier",
            "typed_parameter",
            "default_parameter",
            "typed_default_parameter",
        ],
        excluded_inner: &["list_splat_pattern", "dictionary_splat_pattern"],
        excluded_names: &["self", "cls"],
        exempting_annotation: None,
        names_per_declaration: false,
    },
    test_files: TestFiles {
        name_markers: &[],
        directories: &["tests"],
    },
    decisions: &[
        "if_statement",
        "elif_clause",
        "for_statement",
        "while_statement",
        "except_clause",
    ],
    cases: CaseShape {
        kinds: &["case_clause"],
        default_marker: None,
    },
    logical: LogicalShape {
        kinds: &["boolean_operator"],
        cognitive_operators: &["and", "or"],
        cyclomatic_operators: &[],
    },
    cognitive: CognitiveShape {
        if_kind: "if_statement",
        else_wrapper: None,
        nesting: &[
            "for_statement",
            "while_statement",
            "except_clause",
            "conditional_expression",
            "match_statement",
        ],
        flat: &["elif_clause", "else_clause"],
        labeled_jumps: &[],
    },
    comments: &["comment"],
    parenthesized: &["parenthesized_expression"],
    function_lines: LineSpan::ALL_LINES,
    file_lines: LineSpan::ALL_LINES,
};
