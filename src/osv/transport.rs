use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const CURL: &str = "curl";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(30);
const PARALLEL_TRANSFERS: usize = 16;
const JSON_HEADER: &str = "Content-Type: application/json";
const STANDARD_STREAM: &str = "-";
const BODY_FROM_STDIN: &str = "@-";
const RESPONSE_EXTENSION: &str = "json";
const SCRATCH_PREFIX: &str = "kerbo-osv";

pub trait Transport: std::fmt::Debug {
    fn post_json(&self, url: &str, body: &[u8]) -> Result<Vec<u8>, String>;
    fn get_all(&self, urls: &[String]) -> Result<Vec<Vec<u8>>, String>;
    fn download(&self, url: &str, destination: &Path) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct Curl {
    program: OsString,
    leading: Vec<OsString>,
}

impl Default for Curl {
    fn default() -> Self {
        Self {
            program: OsString::from(CURL),
            leading: Vec::new(),
        }
    }
}

impl Curl {
    #[cfg(all(test, unix))]
    pub fn with_program(program: impl Into<OsString>, leading: Vec<OsString>) -> Self {
        Self {
            program: program.into(),
            leading,
        }
    }

    fn command(&self, timeout: Duration) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.leading)
            .args(["--silent", "--show-error", "--fail", "--location"])
            .arg("--connect-timeout")
            .arg(CONNECT_TIMEOUT.as_secs().to_string())
            .arg("--max-time")
            .arg(timeout.as_secs().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn run(mut command: Command, input: &[u8]) -> Result<Vec<u8>, String> {
        let mut child = command.spawn().map_err(|error| error.to_string())?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input).map_err(|error| error.to_string())?;
        }
        let output = child.wait_with_output().map_err(|error| error.to_string())?;
        checked(output)
    }
}

fn checked(output: Output) -> Result<Vec<u8>, String> {
    if output.status.success() {
        return Ok(output.stdout);
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if detail.is_empty() {
        output.status.to_string()
    } else {
        detail
    })
}

impl Transport for Curl {
    fn post_json(&self, url: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        let mut command = self.command(REQUEST_TIMEOUT);
        command
            .args([
                "--request",
                "POST",
                "--header",
                JSON_HEADER,
                "--data-binary",
                BODY_FROM_STDIN,
            ])
            .arg(url);
        Self::run(command, body)
    }

    fn get_all(&self, urls: &[String]) -> Result<Vec<Vec<u8>>, String> {
        if urls.is_empty() {
            return Ok(Vec::new());
        }
        let scratch = Scratch::new()?;
        let outputs: Vec<PathBuf> = (0..urls.len())
            .map(|index| scratch.path.join(format!("{index}.{RESPONSE_EXTENSION}")))
            .collect();
        let mut config = String::new();
        for (url, output) in urls.iter().zip(&outputs) {
            let _written = writeln!(config, "url = \"{url}\"\noutput = \"{}\"", portable(output));
        }
        let mut command = self.command(REQUEST_TIMEOUT);
        command
            .arg("--parallel")
            .arg("--parallel-max")
            .arg(PARALLEL_TRANSFERS.to_string())
            .args(["--config", STANDARD_STREAM]);
        Self::run(command, config.as_bytes())?;
        outputs
            .iter()
            .map(|output| fs::read(output).map_err(|error| error.to_string()))
            .collect()
    }

    fn download(&self, url: &str, destination: &Path) -> Result<(), String> {
        let mut command = self.command(DOWNLOAD_TIMEOUT);
        command.arg("--output").arg(destination).arg(url);
        Self::run(command, &[]).map(|_| ())
    }
}

fn portable(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[derive(Debug)]
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Result<Self, String> {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{SCRATCH_PREFIX}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).map_err(|error| error.to_string())?;
        Ok(Self { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests;
