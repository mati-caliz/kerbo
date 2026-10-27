use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::lint::go::platform::GoPlatform;

pub const SOURCE: &str = include_str!("helper.go");
// Los demás archivos del paquete `main` del helper, con el nombre con que se escriben.
const OTHER_SOURCES: [(&str, &str); 3] = [
    ("deprecated.go", include_str!("deprecated.go")),
    ("de_morgan.go", include_str!("de_morgan.go")),
    ("stdlib_deprecations.go", include_str!("stdlib_deprecations.go")),
];
const MODULE: &str = "module kerbo-go-types\n\ngo 1.22\n";
const SOURCE_FILE: &str = "main.go";
const MODULE_FILE: &str = "go.mod";
const GO: &str = "go";
const BUILD: &str = "build";
const OUTPUT_FLAG: &str = "-o";
const TAGS_FLAG: &str = "-tags";
const TAG_SEPARATOR: &str = ",";
const PLATFORM_VARIABLES: &[&str] = &["GOOS", "GOARCH", "CGO_ENABLED", "GOFLAGS"];
const OS_VARIABLE: &str = "GOOS";
const ARCH_VARIABLE: &str = "GOARCH";
const CGO_VARIABLE: &str = "CGO_ENABLED";
const CGO_ON: &str = "1";
const CGO_OFF: &str = "0";
const INCOMPLETE_EXIT: i32 = 3;
#[cfg(windows)]
const BINARY: &str = "kerbo-go-types.exe";
#[cfg(not(windows))]
const BINARY: &str = "kerbo-go-types";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// Lo que escribe el helper para el módulo, una línea JSON por archivo, y si se puede guardar: no
// si a algún paquete le faltó lo exportado de una dependencia.
#[derive(Debug)]
pub struct HelperOutput {
    pub lines: String,
    pub complete: bool,
}

pub fn facts(directory: &Path, module_root: &Path, platform: &GoPlatform) -> Result<HelperOutput, String> {
    let binary = built_helper(directory)?;
    let mut command = quiet(Command::new(&binary));
    if !platform.build_tags().is_empty() {
        command
            .arg(TAGS_FLAG)
            .arg(platform.build_tags().join(TAG_SEPARATOR));
    }
    command
        .arg(module_root)
        .current_dir(module_root)
        .env(OS_VARIABLE, platform.os())
        .env(ARCH_VARIABLE, platform.arch())
        .env(CGO_VARIABLE, if platform.cgo() { CGO_ON } else { CGO_OFF });
    let output = command.output().map_err(|error| error.to_string())?;
    let complete = output.status.success();
    if !complete && output.status.code() != Some(INCOMPLETE_EXIT) {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let lines = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    Ok(HelperOutput { lines, complete })
}

// El helper se compila una vez por versión de su fuente, para la máquina, con el `go` del sistema.
fn built_helper(directory: &Path) -> Result<PathBuf, String> {
    let mut hasher = DefaultHasher::new();
    SOURCE.hash(&mut hasher);
    OTHER_SOURCES.hash(&mut hasher);
    let home = directory.join(format!("{:016x}", hasher.finish()));
    let binary = home.join(BINARY);
    if binary.is_file() {
        return Ok(binary);
    }
    fs::create_dir_all(&home).map_err(|error| error.to_string())?;
    fs::write(home.join(SOURCE_FILE), SOURCE).map_err(|error| error.to_string())?;
    for (name, contents) in OTHER_SOURCES {
        fs::write(home.join(name), contents).map_err(|error| error.to_string())?;
    }
    fs::write(home.join(MODULE_FILE), MODULE).map_err(|error| error.to_string())?;
    let mut command = quiet(Command::new(GO));
    for variable in PLATFORM_VARIABLES {
        command.env_remove(variable);
    }
    let output = command
        .args([BUILD, OUTPUT_FLAG])
        .arg(&binary)
        .arg(".")
        .current_dir(&home)
        .env(CGO_VARIABLE, CGO_OFF)
        .output()
        .map_err(|error| format!("{GO}: {error}"))?;
    if output.status.success() {
        Ok(binary)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn quiet(mut command: Command) -> Command {
    command.stdin(Stdio::null());
    hide_window(&mut command);
    command
}

#[cfg(windows)]
fn hide_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
const fn hide_window(_command: &mut Command) {}
