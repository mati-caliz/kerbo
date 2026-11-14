use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::java_encoding;
use crate::limits::{JavaReference, JavaSettings, SourceEncoding};

const BUILD_FILES: [&str; 3] = ["pom.xml", "build.gradle", "build.gradle.kts"];
const CONFIG_FOLDER: &str = "config";
const XML_EXTENSION: &str = ".xml";

#[derive(Debug, Clone, Copy)]
struct Tool {
    name: &'static str,
    build_marks: &'static [&'static str],
}

const PMD: Tool = Tool {
    name: "pmd",
    build_marks: &["maven-pmd-plugin", "'pmd'", "\"pmd\"", "pmd {"],
};
const CHECKSTYLE: Tool = Tool {
    name: "checkstyle",
    build_marks: &[
        "maven-checkstyle-plugin",
        "'checkstyle'",
        "\"checkstyle\"",
        "checkstyle {",
    ],
};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawJava {
    reference: Option<JavaReference>,
    encoding: Option<SourceEncoding>,
}

impl RawJava {
    pub fn resolve(&self, root: &Path) -> JavaSettings {
        JavaSettings {
            reference: self.reference.unwrap_or_else(|| detected_reference(root)),
            encoding: self
                .encoding
                .unwrap_or_else(|| java_encoding::detected(root, &BUILD_FILES)),
        }
    }
}

fn detected_reference(root: &Path) -> JavaReference {
    if uses(root, PMD) && !uses(root, CHECKSTYLE) {
        JavaReference::Pmd
    } else {
        JavaReference::Checkstyle
    }
}

fn uses(root: &Path, tool: Tool) -> bool {
    let in_build = BUILD_FILES.iter().any(|build| {
        fs::read_to_string(root.join(build))
            .is_ok_and(|text| tool.build_marks.iter().any(|mark| text.contains(mark)))
    });
    in_build || root.join(CONFIG_FOLDER).join(tool.name).is_dir() || has_tool_xml(root, tool)
}

fn has_tool_xml(root: &Path, tool: Tool) -> bool {
    fs::read_dir(root).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            name.starts_with(tool.name) && name.ends_with(XML_EXTENSION)
        })
    })
}
