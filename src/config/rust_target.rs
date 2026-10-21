use serde::Deserialize;

// Con qué se evalúan los `#[cfg]` de Rust. Sin nada, la máquina donde corre kerbo y todas las
// features del Cargo.toml, como `cargo clippy --all-features`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustTarget {
    pub target_os: Option<String>,
    pub target_arch: Option<String>,
    pub features: Option<Vec<String>>,
    #[serde(default)]
    pub cfg: Vec<String>,
}
