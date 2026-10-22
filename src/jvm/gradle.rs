use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::classpath::{PARTIAL_EXTENSION, hide_window};

pub const BUILD_FILES: &[&str] = &[
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "gradle.lockfile",
    "libs.versions.toml",
];
const INIT_EXTENSION: &str = "gradle";
const TASK: &str = "kerboClasspath";
const OUTPUT_PROPERTY: &str = "-Dkerbo.classpath.output=";
const INIT_ARGUMENT: &str = "--init-script";
// Silencioso, sin red y sin la caché de configuración (el script lee las configuraciones al correr).
const GRADLE_ARGUMENTS: [&str; 4] = ["-q", "--offline", "--console=plain", "--no-configuration-cache"];
#[cfg(windows)]
const WRAPPER: &str = "gradlew.bat";
#[cfg(not(windows))]
const WRAPPER: &str = "gradlew";
#[cfg(windows)]
const GRADLE: &str = "gradle.bat";
#[cfg(not(windows))]
const GRADLE: &str = "gradle";
#[cfg(not(windows))]
const SHELL: &str = "sh";
// Un script de inicio propio, que no toca el build del proyecto: cada proyecto escribe en una línea
// los jars de sus configuraciones, separados como en el PATH, igual que Maven. La vista tolerante
// saltea lo que no está bajado (un `developmentOnly` que nunca se usó) en vez de fallar.
const INIT_SCRIPT: &str = r#"allprojects { project ->
    project.tasks.register("kerboClasspath") {
        doLast {
            def output = new File(System.getProperty("kerbo.classpath.output"))
            ["testRuntimeClasspath", "runtimeClasspath", "compileClasspath"].each { name ->
                def configuration = project.configurations.findByName(name)
                if (configuration != null && configuration.canBeResolved) {
                    def files = configuration.incoming.artifactView { lenient = true }.files
                    output << files.collect { it.absolutePath }.join(File.pathSeparator) + "\n"
                }
            }
        }
    }
}
"#;

// Le pide a Gradle (el wrapper del proyecto si está) los jars de cada proyecto, sin red.
pub fn ask_gradle(root: &Path, saved: &Path) -> Option<String> {
    let partial = saved.with_extension(PARTIAL_EXTENSION);
    let script = saved.with_extension(INIT_EXTENSION);
    fs::create_dir_all(partial.parent()?).ok()?;
    drop(fs::remove_file(&partial));
    fs::write(&script, INIT_SCRIPT).ok()?;
    let mut command = gradle_command(root);
    command
        .current_dir(root)
        .args(GRADLE_ARGUMENTS)
        .arg(INIT_ARGUMENT)
        .arg(&script)
        .arg(format!("{OUTPUT_PROPERTY}{}", partial.display()))
        .arg(TASK)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_window(&mut command);
    let finished = command.status().ok();
    let contents = fs::read_to_string(&partial).ok();
    drop(fs::remove_file(&partial));
    drop(fs::remove_file(&script));
    finished?.success().then_some(contents).flatten()
}

fn gradle_command(root: &Path) -> Command {
    let wrapper: PathBuf = root.join(WRAPPER);
    if !wrapper.is_file() {
        return Command::new(GRADLE);
    }
    wrapper_command(&wrapper)
}

#[cfg(windows)]
fn wrapper_command(wrapper: &Path) -> Command {
    Command::new(wrapper)
}

// El `gradlew` de un checkout puede no tener el permiso de ejecución: se corre con `sh`.
#[cfg(not(windows))]
fn wrapper_command(wrapper: &Path) -> Command {
    let mut command = Command::new(SHELL);
    command.arg(wrapper);
    command
}
