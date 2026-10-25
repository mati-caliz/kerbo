use std::path::Path;

use tree_sitter::{Node, Point};

use super::Report;
use super::ast::{COMMENT, text};
use crate::rules::PythonRule;

const SHEBANG: &str = "#!";

// EXE001 (shebang en un archivo sin permiso de ejecución) y EXE002 (archivo ejecutable sin shebang,
// que avisa al principio). Como ruff, sólo en Unix y fuera de WSL, donde todo archivo de Windows
// parece ejecutable.
pub fn reports(nodes: &[Node<'_>], source: &[u8], path: Option<&Path>) -> Vec<Report> {
    let Some(executable) = path.and_then(is_executable) else {
        return Vec::new();
    };
    let shebangs: Vec<Node<'_>> = nodes
        .iter()
        .copied()
        .filter(|node| node.kind() == COMMENT && text(*node, source).starts_with(SHEBANG))
        .collect();
    match (shebangs.is_empty(), executable) {
        (true, true) => vec![Report::at_point(PythonRule::EXE002, Point::default(), "")],
        (false, false) => shebangs
            .into_iter()
            .map(|shebang| Report::at(PythonRule::EXE001, shebang, ""))
            .collect(),
        _ => Vec::new(),
    }
}

pub fn is_executable(path: &Path) -> Option<bool> {
    permissions::is_executable(path)
}

#[cfg(unix)]
mod permissions {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::OnceLock;

    const EXECUTABLE_BITS: u32 = 0o111;
    const LINUX: &str = "linux";
    const KERNEL_RELEASE: &str = "/proc/sys/kernel/osrelease";
    const KERNEL_VERSION: &str = "/proc/version";
    const MICROSOFT_KERNEL: &str = "microsoft";
    const CONTAINER_MARKERS: [&str; 2] = ["/.dockerenv", "/run/.containerenv"];
    const CGROUPS: &str = "/proc/self/cgroup";
    const CONTAINER_CGROUPS: [&str; 4] = ["docker", "/container", "/lxc", "kubepods"];

    pub fn is_executable(path: &Path) -> Option<bool> {
        if is_wsl() {
            return None;
        }
        let metadata = path.metadata().ok()?;
        Some(metadata.permissions().mode() & EXECUTABLE_BITS != 0)
    }

    fn is_wsl() -> bool {
        static CACHED: OnceLock<bool> = OnceLock::new();
        *CACHED.get_or_init(detect_wsl)
    }

    // El `is_wsl` de ruff: un kernel de Microsoft que no es el de un contenedor sobre WSL2, porque ahí
    // el sistema de archivos sí tiene permisos de Unix.
    fn detect_wsl() -> bool {
        if std::env::consts::OS != LINUX {
            return false;
        }
        let microsoft_kernel = std::fs::read_to_string(KERNEL_RELEASE)
            .or_else(|_| std::fs::read_to_string(KERNEL_VERSION))
            .is_ok_and(|kernel| kernel.to_ascii_lowercase().contains(MICROSOFT_KERNEL));
        microsoft_kernel && !is_container()
    }

    fn is_container() -> bool {
        if CONTAINER_MARKERS
            .iter()
            .any(|marker| std::fs::metadata(marker).is_ok())
        {
            return true;
        }
        std::fs::read_to_string(CGROUPS).is_ok_and(|cgroups| {
            let cgroups = cgroups.to_ascii_lowercase();
            CONTAINER_CGROUPS.iter().any(|marker| cgroups.contains(marker))
        })
    }
}

#[cfg(not(unix))]
mod permissions {
    use std::path::Path;

    pub const fn is_executable(_path: &Path) -> Option<bool> {
        None
    }
}
