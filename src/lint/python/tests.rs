mod fix_parity;
mod fixtures;

use std::path::Path;

use tree_sitter::Parser;

use super::{findings, relaxed_by};
use crate::fix::apply;
use crate::language::Language;
use crate::limits::{Limits, Preset};
use crate::lint::{LintFinding, LintKind};
use crate::rules::{PythonRule, RuleId};
use crate::test_support::TempTree;
use fixtures::FIXTURES;

// Arreglos que, como en ruff, pueden dejar el mismo aviso en el mismo lugar para la vuelta siguiente:
// `((x))` pierde un par de paréntesis por vuelta y tres `with` anidados se juntan de a dos.
const REPEATED_FIXES: [LintKind; 2] = [
    LintKind::Python(PythonRule::UP034),
    LintKind::Python(PythonRule::SIM117),
];

fn strict() -> Limits {
    Preset::Strict.limits(Language::Python, false)
}

fn lints_at(path: &str, source: &str, limits: &Limits) -> Vec<LintFinding> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    findings(tree.root_node(), source.as_bytes(), limits, Some(Path::new(path)))
}

fn lints(source: &str) -> Vec<LintFinding> {
    lints_at("module.py", source, &strict())
}

fn rendered(findings: &[LintFinding]) -> Vec<String> {
    let mut lines: Vec<String> = findings
        .iter()
        .map(|finding| {
            format!(
                "{}:{} {}",
                finding.line,
                finding.column,
                finding.kind.rule().as_str().to_uppercase()
            )
        })
        .collect();
    lines.sort();
    lines
}

fn expected(golden: &str) -> Vec<String> {
    let mut lines: Vec<String> = golden.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

fn fixed(source: &str, rule: PythonRule) -> String {
    let found = lints(source);
    let fix = found
        .iter()
        .find(|finding| finding.kind == LintKind::Python(rule))
        .and_then(|finding| finding.fix.clone())
        .expect("trae arreglo");
    apply(&fix.edits, source).expect("el arreglo cae dentro del archivo")
}

#[test]
fn reports_exactly_what_ruff_reports_with_its_default_rules() {
    for (path, source, golden) in FIXTURES {
        assert_eq!(
            rendered(&lints_at(path, source, &strict())),
            expected(golden),
            "{path} contra ruff 0.17 (E4, E7, E9, F y las del conjunto nuevo)"
        );
    }
}

#[test]
fn every_fix_removes_its_own_problem() {
    for (path, source, _) in FIXTURES {
        let before = lints_at(path, source, &strict());
        for finding in before
            .iter()
            .filter(|finding| finding.fix.is_some() && !REPEATED_FIXES.contains(&finding.kind))
        {
            let edits = finding
                .fix
                .as_ref()
                .map(|fix| fix.edits.clone())
                .unwrap_or_default();
            let after = apply(&edits, source).expect("el arreglo cae dentro del archivo");
            let count =
                |findings: &[LintFinding]| findings.iter().filter(|other| other.kind == finding.kind).count();
            let fixed = lints_at(path, &after, &strict());
            let still_there = fixed.iter().any(|other| {
                other.kind == finding.kind
                    && other.line == finding.line
                    && other.column == finding.column
                    && other.subject == finding.subject
            });
            let resolved = !still_there || count(&fixed) < count(&before);
            assert!(resolved, "{path}: el arreglo de {finding:?} no lo resuelve");
        }
    }
}

#[test]
fn mechanical_fixes_rewrite_only_what_the_rule_points_at() {
    assert_eq!(
        fixed("import os\nimport sys\nprint(sys)\n", PythonRule::F401),
        "import sys\nprint(sys)\n"
    );
    assert_eq!(
        fixed("from a import b, c, d\nprint(c)\n", PythonRule::F401),
        "from a import c\nprint(c)\n"
    );
    assert_eq!(
        fixed("from a import (\n    b,\n    c,\n)\nprint(c)\n", PythonRule::F401),
        "from a import (\n    c,\n)\nprint(c)\n"
    );
    assert_eq!(
        fixed("if x:\n    import os\n", PythonRule::F401),
        "if x:\n    pass\n"
    );
    assert_eq!(fixed("x = f\"{{hola}}\"\n", PythonRule::F541), "x = \"{hola}\"\n");
    assert_eq!(
        fixed("if x==None:\n    pass\n", PythonRule::E711),
        "if x is None:\n    pass\n"
    );
    assert_eq!(
        fixed("if x != True:\n    pass\n", PythonRule::E712),
        "if not x:\n    pass\n"
    );
    assert_eq!(
        fixed("if not x in y:\n    pass\n", PythonRule::E713),
        "if x not in y:\n    pass\n"
    );
    assert_eq!(
        fixed("if not x is y:\n    pass\n", PythonRule::E714),
        "if x is not y:\n    pass\n"
    );
    assert_eq!(
        fixed("if x is 1:\n    pass\n", PythonRule::F632),
        "if x == 1:\n    pass\n"
    );
    assert_eq!(fixed("x = 1;\n", PythonRule::E703), "x = 1\n");
    assert_eq!(
        fixed("raise NotImplemented\n", PythonRule::F901),
        "raise NotImplementedError\n"
    );
    assert_eq!(
        fixed(
            "try:\n    pass\nexcept ValueError as error:\n    pass\n",
            PythonRule::F841
        ),
        "try:\n    pass\nexcept ValueError:\n    pass\n"
    );
}

#[test]
fn imports_sharing_a_line_lose_their_separator_and_in_a_package_have_no_fix() {
    let shared = lints("import os; x = 1\n");
    assert_eq!(rendered(&shared), ["1:1 I001", "1:10 E702", "1:8 F401"]);
    assert_eq!(fixed("import os; x = 1\n", PythonRule::F401), "x = 1\n");
    let package = lints_at("pkg/__init__.py", "import os\n", &strict());
    assert_eq!(rendered(&package), ["1:8 F401"]);
    assert_eq!(package.first().and_then(|finding| finding.fix.clone()), None);
}

#[test]
fn noqa_directives_silence_like_ruff() {
    let source = "import a  # noqa\nimport b  # NoQA: F401, E501\nimport c  # noqa: E501\nimport d  # noqa:\nx = None == \"\"\"\n\"\"\"  # noqa: E711\ny = None == \\\n    a  # noqa: E711\nz = None == \\\n    a\n";
    assert_eq!(
        rendered(&lints(source)),
        [
            "2:11 RUF100",
            "3:11 RUF100",
            "3:8 F401",
            "4:8 F401",
            "5:5 PLR0133",
            "6:6 RUF100",
            "9:5 E711"
        ]
    );
    assert_eq!(
        rendered(&lints("# ruff: noqa\nimport os\n")),
        Vec::<String>::new()
    );
    assert_eq!(
        rendered(&lints("# flake8: noqa: F401\nimport os\nx = 1;\n")),
        ["2:1 I001", "3:6 E703"]
    );
}

#[test]
fn files_tree_sitter_cannot_parse_keep_only_the_local_rules() {
    let source = "import os\nlazy import json\nx = 1;\n";
    assert_eq!(rendered(&lints(source)), ["3:6 E703"]);
}

#[test]
fn unparsed_files_only_flag_noqa_codes_that_do_not_need_findings() {
    let source = "import os  # noqa: F401\nlazy import json  # noqa: E501\nx = 1  # noqa\n";
    assert_eq!(rendered(&lints(source)), ["2:19 RUF100"]);
}

#[test]
fn presets_and_switches_turn_rules_off() {
    let source = "import os\nx = 1;\n";
    let minimal = Preset::Minimal.limits(Language::Python, false);
    assert_eq!(rendered(&lints_at("module.py", source, &minimal)), ["1:8 F401"]);
    let without = strict().with_rule(RuleId::Python(PythonRule::F401), None);
    assert_eq!(
        rendered(&lints_at("module.py", source, &without)),
        ["1:1 I001", "2:6 E703"]
    );
    let everything_off = PythonRule::ALL.into_iter().fold(strict(), |limits, rule| {
        limits.with_rule(RuleId::Python(rule), None)
    });
    assert_eq!(lints_at("module.py", source, &everything_off), Vec::new());
    assert_eq!(relaxed_by(Preset::Recommended), Vec::<RuleId>::new());
    assert!(relaxed_by(Preset::Minimal).contains(&RuleId::Python(PythonRule::E741)));
}

#[test]
fn reports_name_the_offending_symbol() {
    let found = lints("import os.path as p\nprint(missing)\n");
    let subjects: Vec<(&str, &str)> = found
        .iter()
        .map(|finding| (finding.kind.rule().as_str(), finding.subject.as_str()))
        .collect();
    assert_eq!(subjects, [("i001", ""), ("f401", "os.path"), ("f821", "missing")]);
}

#[test]
fn columns_count_bytes_like_the_rest_of_the_rules() {
    let found = lints("x = \"ñ\"; import os\n");
    assert_eq!(
        rendered(&found),
        ["1:11 E402", "1:11 I001", "1:18 F401", "1:9 E702"]
    );
}

#[test]
fn module_names_are_checked_only_inside_packages() {
    let tree = TempTree::new("python-module-names");
    let files = [
        "pkg/__init__.py",
        "pkg/BadName.py",
        "pkg/good_name.py",
        "pkg/migrations/__init__.py",
        "pkg/migrations/0001_initial.py",
        "pkg/tests/__init__.py",
        "pkg/tests/0001_initial.py",
        "loose/Loose.py",
        "Upper/__init__.py",
        "Upper/inner/__init__.py",
    ];
    for file in files {
        tree.write(file, "");
    }
    let invalid: Vec<String> = files
        .iter()
        .filter(|file| {
            let path = tree.path().join(file);
            lints_at(
                &path.to_string_lossy(),
                "x = 1
",
                &strict(),
            )
            .iter()
            .any(|finding| finding.kind == LintKind::Python(PythonRule::N999))
        })
        .map(|file| (*file).to_owned())
        .collect();
    assert_eq!(
        invalid,
        [
            "pkg/BadName.py",
            "pkg/tests/0001_initial.py",
            "Upper/__init__.py",
            "Upper/inner/__init__.py"
        ]
    );
}

#[test]
fn self_imports_are_found_from_the_package_path() {
    let tree = TempTree::new("python-import-self");
    tree.write("pkg/__init__.py", "");
    tree.write("pkg/sub/__init__.py", "");
    let module = "import pkg.sub.mod
import pkg.sub.mod as alias
import pkg.sub
from pkg.sub import mod
from . import mod as again
from .. import sub
from .mod import thing
from ... import other
import os
";
    let found = |relative: &str, source: &str| {
        let path = tree.path().join(relative);
        let mut lines = rendered(&lints_at(&path.to_string_lossy(), source, &strict()));
        lines.retain(|line| line.ends_with("PLW0406"));
        lines
    };
    assert_eq!(
        found("pkg/sub/mod.py", module),
        ["1:8 PLW0406", "2:8 PLW0406", "4:21 PLW0406", "5:15 PLW0406"]
    );
    let quiet = [
        ("pkg/sub/__init__.py", "import pkg.sub\nfrom pkg import sub\n"),
        ("loose.py", "import loose\n"),
    ];
    for (relative, source) in quiet {
        assert_eq!(found(relative, source), Vec::<String>::new(), "{relative}");
    }
}

#[cfg(unix)]
#[test]
fn shebangs_are_checked_against_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt;

    const EXECUTABLE: u32 = 0o755;
    const PLAIN: u32 = 0o644;
    let tree = TempTree::new("python-executable-bits");
    let cases = [
        (
            "script.py",
            "#!/usr/bin/env python\nx = 1\n",
            EXECUTABLE,
            Vec::<&str>::new(),
        ),
        (
            "plain.py",
            "#!/usr/bin/env python\nx = 1\n",
            PLAIN,
            vec!["1:1 EXE001"],
        ),
        ("run.py", "x = 1\n", EXECUTABLE, vec!["1:1 EXE002"]),
        ("module.py", "x = 1\n", PLAIN, Vec::new()),
    ];
    for (relative, source, mode, wanted) in cases {
        tree.write(relative, source);
        let path = tree.path().join(relative);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            .expect("se cambian los permisos");
        let mut lines = rendered(&lints_at(&path.to_string_lossy(), source, &strict()));
        lines.retain(|line| line.ends_with("EXE001") || line.ends_with("EXE002"));
        assert_eq!(lines, wanted, "{relative}");
    }
}
