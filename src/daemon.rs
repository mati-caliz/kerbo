mod client;
mod endpoint;
mod protocol;
mod server;

use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::thread;
use std::time::Duration;

pub use client::{Forwarded, Invocation, deliver, forward};
use protocol::{Request, Response};
pub use server::{Lifetime, serve};

use crate::cache::{cache_root, runtime_identity};
use crate::i18n::{Notice, Problem};

const IDLE_TIMEOUT: Duration = Duration::from_mins(30);
const STARTUP_POLL: Duration = Duration::from_millis(50);
const STARTUP_ATTEMPTS: usize = 100;
const DAEMON_COMMAND: &str = "daemon";
const RUN_ACTION: &str = "run";
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

pub fn endpoint_directory() -> Option<PathBuf> {
    cache_root()
}

pub fn default_lifetime() -> Lifetime {
    Lifetime {
        idle_timeout: IDLE_TIMEOUT,
        identity: runtime_identity(),
    }
}

pub fn status(directory: &Path) -> Notice {
    match client::ask(directory, Request::Status) {
        Some((endpoint, Response::Running { pid })) => Notice::DaemonRunning {
            pid,
            port: endpoint.port,
        },
        Some(_) | None => Notice::DaemonNotRunning,
    }
}

pub fn stop(directory: &Path) -> Notice {
    match client::ask(directory, Request::Stop) {
        Some((_, Response::Stopping)) => Notice::DaemonStopped,
        Some(_) | None => Notice::DaemonNotRunning,
    }
}

pub fn start(directory: &Path) -> Result<Notice, Problem> {
    start_with(directory, spawn_detached)
}

fn start_with(directory: &Path, launch: impl FnOnce() -> io::Result<()>) -> Result<Notice, Problem> {
    if let Notice::DaemonRunning { pid, port } = status(directory) {
        return Ok(Notice::DaemonAlreadyRunning { pid, port });
    }
    launch().map_err(|error| Problem::DaemonUnavailable(error.to_string()))?;
    for _ in 0..STARTUP_ATTEMPTS {
        if let Notice::DaemonRunning { pid, port } = status(directory) {
            return Ok(Notice::DaemonStarted { pid, port });
        }
        thread::sleep(STARTUP_POLL);
    }
    Err(Problem::DaemonUnavailable(directory.display().to_string()))
}

fn spawn_detached() -> io::Result<()> {
    let mut process = Process::new(env::current_exe()?);
    process
        .args([DAEMON_COMMAND, RUN_ACTION])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut process);
    process.spawn().map(drop)
}

#[cfg(unix)]
fn detach(process: &mut Process) {
    use std::os::unix::process::CommandExt;
    process.process_group(0);
}

#[cfg(windows)]
fn detach(process: &mut Process) {
    use std::os::windows::process::CommandExt;
    process.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
const fn detach(_process: &mut Process) {}

#[cfg(test)]
mod tests;
