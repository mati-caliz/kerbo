use std::path::{Component, Path};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::deploy;
use crate::profile::{self, Profile};
use crate::shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Go,
    Python,
    Java,
    TypeScript,
    Tsx,
    JavaScript,
    Rust,
    Sql,
    Dockerfile,
    Compose,
    Shell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stack {
    Typescript,
    Python,
    Java,
    Go,
    Rust,
    Sql,
    Docker,
    Shell,
}

impl Language {
    pub const fn stack(self) -> Stack {
        match self {
            Self::Go => Stack::Go,
            Self::Python => Stack::Python,
            Self::Java => Stack::Java,
            Self::TypeScript | Self::Tsx | Self::JavaScript => Stack::Typescript,
            Self::Rust => Stack::Rust,
            Self::Sql => Stack::Sql,
            Self::Dockerfile | Self::Compose => Stack::Docker,
            Self::Shell => Stack::Shell,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        deploy::detect(path)
            .or_else(|| Self::from_extension(path))
            .or_else(|| shell::detect(path))
    }

    fn from_extension(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "go" => Some(Self::Go),
            "py" | "pyi" => Some(Self::Python),
            "java" => Some(Self::Java),
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "rs" => Some(Self::Rust),
            _ => None,
        }
    }

    pub fn grammar(self) -> Option<tree_sitter::Language> {
        let grammar = match self {
            Self::Go => tree_sitter_go::LANGUAGE,
            Self::Python => tree_sitter_python::LANGUAGE,
            Self::Java => tree_sitter_java::LANGUAGE,
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
            Self::JavaScript => tree_sitter_javascript::LANGUAGE,
            Self::Rust => tree_sitter_rust::LANGUAGE,
            Self::Compose => tree_sitter_yaml::LANGUAGE,
            Self::Shell => tree_sitter_bash::LANGUAGE,
            Self::Sql | Self::Dockerfile => return None,
        };
        Some(grammar.into())
    }

    pub fn is_test_path(self, relative_path: &Path) -> bool {
        let Some(profile) = self.profile() else {
            return false;
        };
        let test_files = &profile.test_files;
        let file_name = relative_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let marked = test_files
            .name_markers
            .iter()
            .any(|marker| file_name.contains(marker));
        marked
            || relative_path.components().any(|component| match component {
                Component::Normal(directory) => directory
                    .to_str()
                    .is_some_and(|directory| test_files.directories.contains(&directory)),
                Component::Prefix(_) | Component::RootDir | Component::CurDir | Component::ParentDir => false,
            })
    }

    pub const fn profile(self) -> Option<&'static Profile> {
        match self {
            Self::Go => Some(&profile::GO),
            Self::Python => Some(&profile::PYTHON),
            Self::Java => Some(&profile::JAVA),
            Self::TypeScript | Self::Tsx | Self::JavaScript => Some(&profile::ECMASCRIPT),
            Self::Rust => Some(&profile::RUST),
            Self::Sql | Self::Dockerfile | Self::Compose | Self::Shell => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Language, Stack};

    #[test]
    fn detects_language_by_extension() {
        assert_eq!(Language::from_path(Path::new("a/b.go")), Some(Language::Go));
        assert_eq!(Language::from_path(Path::new("x.pyi")), Some(Language::Python));
        assert_eq!(Language::from_path(Path::new("X.java")), Some(Language::Java));
        assert_eq!(
            Language::from_path(Path::new("x.mts")),
            Some(Language::TypeScript)
        );
        assert_eq!(Language::from_path(Path::new("x.tsx")), Some(Language::Tsx));
        assert_eq!(
            Language::from_path(Path::new("x.cjs")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_path(Path::new("src/main.rs")),
            Some(Language::Rust)
        );
        assert_eq!(Language::from_path(Path::new("README.md")), None);
        assert_eq!(Language::from_path(Path::new("Makefile")), None);
        assert_eq!(
            Language::from_path(Path::new("prisma/migrations/1_init/migration.sql")),
            Some(Language::Sql)
        );
        assert_eq!(
            Language::from_path(Path::new("db/V2__add_index.sql")),
            Some(Language::Sql)
        );
        assert_eq!(Language::from_path(Path::new("seed/data.sql")), None);
        assert_eq!(
            Language::from_path(Path::new("api/Dockerfile.prod")),
            Some(Language::Dockerfile)
        );
        assert_eq!(
            Language::from_path(Path::new("web.Dockerfile")),
            Some(Language::Dockerfile)
        );
        assert_eq!(Language::from_path(Path::new("Dockerfile.dockerignore")), None);
        assert_eq!(
            Language::from_path(Path::new("docker-compose.prod.yml")),
            Some(Language::Compose)
        );
        assert_eq!(
            Language::from_path(Path::new("compose.yaml")),
            Some(Language::Compose)
        );
        assert_eq!(
            Language::from_path(Path::new("docker-compose-ci.yaml")),
            Some(Language::Compose)
        );
        assert_eq!(Language::from_path(Path::new("compose-config.yml")), None);
        assert_eq!(Language::from_path(Path::new("config.yml")), None);
    }

    #[test]
    fn recognizes_test_files_like_the_standard() {
        assert!(Language::TypeScript.is_test_path(Path::new("src/a.test.ts")));
        assert!(Language::Tsx.is_test_path(Path::new("src/__tests__/a.tsx")));
        assert!(Language::JavaScript.is_test_path(Path::new("e2e/flow.js")));
        assert!(!Language::TypeScript.is_test_path(Path::new("src/testing.ts")));
        assert!(Language::Python.is_test_path(Path::new("tests/test_a.py")));
        assert!(!Language::Python.is_test_path(Path::new("app/test_helpers.py")));
        assert!(Language::Go.is_test_path(Path::new("pkg/a_test.go")));
        assert!(Language::Java.is_test_path(Path::new("src/test/java/ATest.java")));
        assert!(!Language::Java.is_test_path(Path::new("/src/main/java/A.java")));
        assert!(Language::Rust.is_test_path(Path::new("tests/cli.rs")));
        assert!(!Language::Rust.is_test_path(Path::new("src/lib.rs")));
    }

    #[test]
    fn groups_languages_by_the_stacks_of_the_standard() {
        assert_eq!(Language::JavaScript.stack(), Stack::Typescript);
        assert_eq!(Language::Tsx.stack(), Stack::Typescript);
        assert_eq!(Language::Python.stack(), Stack::Python);
        assert_eq!(Language::Java.stack(), Stack::Java);
        assert_eq!(Language::Go.stack(), Stack::Go);
        assert_eq!(Language::Rust.stack(), Stack::Rust);
        assert_eq!(Language::Sql.stack(), Stack::Sql);
        assert_eq!(Language::Compose.stack(), Stack::Docker);
    }

    #[test]
    fn every_grammar_loads() {
        for language in [
            Language::Go,
            Language::Python,
            Language::Java,
            Language::TypeScript,
            Language::Tsx,
            Language::JavaScript,
            Language::Rust,
            Language::Compose,
        ] {
            let mut parser = tree_sitter::Parser::new();
            let grammar = language.grammar().expect("tiene gramática de tree-sitter");
            assert!(parser.set_language(&grammar).is_ok(), "{language:?}");
        }
        assert!(Language::Sql.grammar().is_none());
        assert!(!Language::Dockerfile.is_test_path(Path::new("tests/Dockerfile")));
    }
}
