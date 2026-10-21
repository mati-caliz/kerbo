use std::env;
use std::io::{self, Cursor};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::endpoint::Endpoint;
use super::protocol::{Envelope, Finished, Request, Response, RunRequest, receive, send};
use crate::app::{Terminal, run};
use crate::cli::{self, Command};
use crate::i18n::LangPreference;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_CHECK: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
pub struct Lifetime {
    pub idle_timeout: Duration,
    pub identity: u64,
}

#[derive(Debug)]
struct Activity {
    last: Mutex<Instant>,
    stopping: AtomicBool,
}

pub fn serve(directory: &Path, lifetime: Lifetime) -> io::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let endpoint = Endpoint::new(listener.local_addr()?.port(), lifetime.identity);
    endpoint.publish(directory)?;
    let activity = Arc::new(Activity {
        last: Mutex::new(Instant::now()),
        stopping: AtomicBool::new(false),
    });
    watch_idleness(Arc::clone(&activity), &endpoint, lifetime.idle_timeout);
    for stream in listener.incoming() {
        if activity.stopping.load(Ordering::SeqCst) {
            break;
        }
        let Ok(mut stream) = stream else {
            continue;
        };
        let stop = handle(&mut stream, &endpoint.token);
        *activity.last.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
        if stop {
            break;
        }
    }
    endpoint.withdraw(directory);
    Ok(())
}

fn watch_idleness(activity: Arc<Activity>, endpoint: &Endpoint, idle_timeout: Duration) {
    let address = endpoint.address();
    let check = IDLE_CHECK.min(idle_timeout);
    thread::spawn(move || {
        loop {
            thread::sleep(check);
            let idle = activity
                .last
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed();
            if idle >= idle_timeout {
                break;
            }
        }
        activity.stopping.store(true, Ordering::SeqCst);
        let _wake = TcpStream::connect(address);
    });
}

fn handle(stream: &mut TcpStream, token: &str) -> bool {
    if stream.set_read_timeout(Some(READ_TIMEOUT)).is_err() {
        return false;
    }
    let envelope: Option<Envelope> = receive(stream).ok();
    let (response, stop) = match envelope {
        Some(envelope) if envelope.token == token => respond(envelope.request),
        Some(_) | None => (Response::Refused, false),
    };
    let _sent = send(stream, &response);
    stop
}

fn respond(request: Request) -> (Response, bool) {
    match request {
        Request::Run(run_request) => (execute(&run_request), false),
        Request::Status => (Response::Running { pid: process::id() }, false),
        Request::Stop => (Response::Stopping, true),
    }
}

fn execute(request: &RunRequest) -> Response {
    let preference = LangPreference {
        flag: request.flag,
        locale: request.locale,
    };
    let Ok(parsed) = cli::try_parse(preference.resolve(None), &request.arguments) else {
        return Response::Refused;
    };
    if !matches!(parsed.command, Command::Check(_) | Command::Hook(_))
        || env::set_current_dir(&request.directory).is_err()
    {
        return Response::Refused;
    }
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
    Response::Finished(Finished {
        report: String::from_utf8(report).unwrap_or_default(),
        log: String::from_utf8(log).unwrap_or_default(),
        status,
    })
}
