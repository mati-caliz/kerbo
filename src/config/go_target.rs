use serde::Deserialize;

// Con qué plataforma se eligen los archivos de Go que miran las reglas de golangci-lint. Sin nada,
// `GOOS` y `GOARCH` del entorno o la máquina donde corre kerbo, como golangci-lint.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoTarget {
    pub goos: Option<String>,
    pub goarch: Option<String>,
    #[serde(default)]
    pub build_tags: Vec<String>,
    // `linters.settings.errcheck.exclude-functions`: se suman a las exclusiones de fábrica de errcheck.
    #[serde(default)]
    pub errcheck_exclude_functions: Vec<String>,
    // `check-blank`: también los errores asignados a `_`.
    #[serde(default)]
    pub errcheck_check_blank: bool,
    // `check-type-assertions`: también las aserciones de tipo sin mirar el `ok`.
    #[serde(default)]
    pub errcheck_check_type_assertions: bool,
}
