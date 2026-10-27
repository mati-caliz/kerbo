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
}
