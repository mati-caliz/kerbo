use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Cursor};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::endpoint::Endpoint;
use super::protocol::{Envelope, Finished, Request, Response, RunRequest, receive, send};
use super::workspace::Workspace;
use crate::app::{Terminal, run};
use crate::cli::{self, Command};
use crate::i18n::LangPreference;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_CHECK: Duration = Duration::from_secs(30);
const INSTANCE_LOCK: &str = "daemon.lock";

#[derive(Debug, Clone)]
pub struct Lifetime {
    pub idle_timeout: Duration,
    pub identity: u64,
    pub home: PathBuf,
}

#[derive(Debug)]
struct Shared {
    token: String,
    address: SocketAddr,
    workspace: Workspace,
    last_activity: Mutex<Instant>,
    in_flight: AtomicUsize,
    stopping: AtomicBool,
}

impl Shared {
    fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _wake = TcpStream::connect(self.address);
    }
}

pub fn serve(directory: &Path, lifetime: &Lifetime) -> io::Result<()> {
    let Some(_instance) = claim_instance(directory)? else {
        return Ok(());
    };
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let endpoint = Endpoint::new(listener.local_addr()?.port(), lifetime.identity);
    endpoint.publish(directory)?;
    let shared = Arc::new(Shared {
        token: endpoint.token.clone(),
        address: endpoint.address(),
        workspace: Workspace::new(&lifetime.home),
        last_activity: Mutex::new(Instant::now()),
        in_flight: AtomicUsize::new(0),
        stopping: AtomicBool::new(false),
    });
    watch_idleness(Arc::clone(&shared), lifetime.idle_timeout);
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    for stream in listener.incoming() {
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else {
            continue;
        };
        workers.retain(|worker| !worker.is_finished());
        let worker_shared = Arc::clone(&shared);
        workers.push(thread::spawn(move || attend(stream, &worker_shared)));
    }
    endpoint.withdraw(directory);
    for worker in workers {
        let _joined = worker.join();
    }
    Ok(())
}

fn claim_instance(directory: &Path) -> io::Result<Option<File>> {
    fs::create_dir_all(directory)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join(INSTANCE_LOCK))?;
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

fn watch_idleness(shared: Arc<Shared>, idle_timeout: Duration) {
    let check = IDLE_CHECK.min(idle_timeout);
    thread::spawn(move || {
        loop {
            thread::sleep(check);
            let idle = shared
                .last_activity
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed();
            if shared.in_flight.load(Ordering::SeqCst) == 0 && idle >= idle_timeout {
                break;
            }
        }
        shared.stop();
    });
}

fn attend(mut stream: TcpStream, shared: &Shared) {
    shared.in_flight.fetch_add(1, Ordering::SeqCst);
    let stop = handle(&mut stream, shared);
    *shared
        .last_activity
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Instant::now();
    shared.in_flight.fetch_sub(1, Ordering::SeqCst);
    if stop {
        shared.stop();
    }
}

fn handle(stream: &mut TcpStream, shared: &Shared) -> bool {
    if stream.set_read_timeout(Some(READ_TIMEOUT)).is_err() {
        return false;
    }
    let envelope: Option<Envelope> = receive(stream).ok();
    let (response, stop) = match envelope {
        Some(envelope) if envelope.token == shared.token => respond(envelope.request, &shared.workspace),
        Some(_) | None => (Response::Refused, false),
    };
    let _sent = send(stream, &response);
    stop
}

fn respond(request: Request, workspace: &Workspace) -> (Response, bool) {
    match request {
        Request::Run(run_request) => (execute(&run_request, workspace), false),
        Request::Status => (Response::Running { pid: process::id() }, false),
        Request::Stop => (Response::Stopping, true),
    }
}

fn execute(request: &RunRequest, workspace: &Workspace) -> Response {
    let preference = LangPreference {
        flag: request.flag,
        locale: request.locale,
    };
    let Ok(parsed) = cli::try_parse(preference.resolve(None), &request.arguments) else {
        return Response::Refused;
    };
    if !matches!(parsed.command, Command::Check(_) | Command::Hook(_)) {
        return Response::Refused;
    }
    let Ok(visit) = workspace.enter(&request.directory) else {
        return Response::Refused;
    };
    let mut input = Cursor::new(request.input.as_bytes());
    let mut report = Vec::new();
    let mut log = Vec::new();
    let status = run(
        &parsed.command,
        preference,
        &request.directory,
        &mut Terminal {
            input: &mut input,
            report: &mut report,
            log: &mut log,
        },
    );
    drop(visit);
    Response::Finished(Finished {
        report: String::from_utf8(report).unwrap_or_default(),
        log: String::from_utf8(log).unwrap_or_default(),
        status,
    })
}
