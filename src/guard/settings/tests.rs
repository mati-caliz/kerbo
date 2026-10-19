use super::loosened_lines;
use crate::diff::Changes;

fn loosened(previous: &str, current: &str, relative: &str) -> Vec<(usize, String)> {
    loosened_lines(current, previous, &Changes::Whole, relative)
        .into_iter()
        .map(|loosening| (loosening.line, loosening.new.shown))
        .collect()
}

#[test]
fn reports_ceilings_raised_and_floors_lowered() {
    let golangci = loosened(
        "    cyclop:\n      max-complexity: 10\n    funlen:\n      lines: 80\n",
        "    cyclop:\n      max-complexity: 15\n    funlen:\n      lines: 60\n",
        ".golangci.yml",
    );
    assert_eq!(
        golangci,
        [(2, "max-complexity: 15".to_owned())],
        "bajar un máximo es endurecer"
    );
    let eslint = loosened(
        "  \"id-length\": [\"error\", { min: 2 }],\n  \"no-console\": \"error\",\n",
        "  \"id-length\": [\"error\", { min: 1 }],\n  \"no-console\": \"off\",\n",
        "eslint.config.mjs",
    );
    assert_eq!(eslint.len(), 2, "un mínimo que baja y una regla apagada");
    let checkstyle = loosened(
        "<property name=\"max\" value=\"80\"/>\n",
        "<property name=\"max\" value=\"120\"/>\n",
        "checkstyle.xml",
    );
    assert_eq!(checkstyle.len(), 1);
    let other_module = loosened(
        "    <module name=\"ParameterNumber\">\n        <property name=\"max\" value=\"10\"/>\n    </module>\n",
        "    <module name=\"FileLength\">\n        <property name=\"max\" value=\"500\"/>\n    </module>\n",
        "checkstyle.xml",
    );
    assert_eq!(
        other_module,
        Vec::new(),
        "el max de otro módulo no es el mismo umbral"
    );
}

#[test]
fn reports_coverage_strictness_and_presets() {
    let vitest = loosened(
        "      thresholds: { lines: 80, branches: 70 },\n",
        "      thresholds: { lines: 80, branches: 60 },\n",
        "vitest.config.ts",
    );
    assert_eq!(vitest.len(), 1, "en la cobertura, bajar es aflojar");
    let tsconfig = loosened(
        "    \"strict\": true,\n    \"noImplicitAny\": true,\n",
        "    \"strict\": false,\n    \"noImplicitAny\": false,\n",
        "tsconfig.json",
    );
    assert_eq!(tsconfig.len(), 2);
    let kerbo = loosened("preset = \"strict\"\n", "preset = \"minimal\"\n", "kerbo.toml");
    assert_eq!(kerbo.len(), 1);
}

#[test]
fn ignores_tightening_and_unrelated_numbers() {
    let tightened = loosened(
        "max-complexity: 15\ntimeout: 10m\nversion: \"1.2.3\"\nskipComments: true\n",
        "max-complexity: 10\ntimeout: 20m\nversion: \"1.2.4\"\nskipComments: false\n",
        ".golangci.yml",
    );
    assert_eq!(tightened, Vec::new());
}
