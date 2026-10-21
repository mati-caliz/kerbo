use std::env;
use std::io::{Cursor, empty};
use std::net::TcpStream;
use std::path::Path;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::client::{Forwarded, Invocation, ask, deliver, forward};
use super::endpoint::Endpoint;
use super::protocol::{Envelope, Request, Response, receive, send};
use super::server::{Lifetime, serve};
use super::{
    IDLE_TIMEOUT, default_lifetime, endpoint_directory, ensure_running_with, start_with, status, stop,
};
use crate::app::{Terminal, run};
use crate::cli::{self, Cli};
use crate::i18n::{Lang, LangPreference, Notice, Problem};
use crate::test_support::TempTree;

const IDENTITY: u64 = 42;
const LONG_IDLE: Duration = Duration::from_mins(5);
const SHORT_IDLE: Duration = Duration::from_millis(100);
const STARTUP_POLL: Duration = Duration::from_millis(10);
const LONG_FUNCTION: &str =
    "def build(first, second, third, fourth, fifth, sixth, seventh):\n    return first\n";
const PREFERENCE: LangPreference = LangPreference {
    flag: None,
    locale: Lang::Spanish,
};

fn start_server(directory: &Path, idle_timeout: Duration) -> JoinHandle<()> {
    let owned = directory.to_path_buf();
    let handle = thread::spawn(move || {
        serve(
            &owned,
            Lifetime {
                idle_timeout,
                identity: IDENTITY,
            },
        )
        .expect("el demonio arranca");
    });
    while Endpoint::read(directory).is_none() {
        thread::sleep(STARTUP_POLL);
    }
    handle
}

fn parsed(arguments: &[String]) -> Cli {
    cli::try_parse(Lang::Spanish, arguments).expect("los argumentos se entienden")
}

fn check_arguments(file: &Path) -> Vec<String> {
    ["kerbo", "check", "--no-cache", "--format", "json"]
        .iter()
        .map(ToString::to_string)
        .chain([file.display().to_string()])
        .collect()
}

fn local_run(cli: &Cli, input: &str) -> (String, String, u8) {
    let mut report = Vec::new();
    let mut log = Vec::new();
    let directory = env::current_dir().expect("hay directorio actual");
    let exit = run(
        &cli.command,
        PREFERENCE,
        &directory,
        &mut Terminal {
            input: &mut Cursor::new(input.as_bytes()),
            report: &mut report,
            log: &mut log,
        },
    );
    (
        String::from_utf8(report).expect("es texto"),
        String::from_utf8(log).expect("es texto"),
        exit,
    )
}

fn without_timing(log: &str) -> String {
    log.lines()
        .map(|line| line.rsplit_once(", ").map_or(line, |(counts, _)| counts))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_check_through_the_daemon_reports_exactly_what_a_local_one_does() {
    let tree = TempTree::new("daemon-check");
    tree.write("long.py", LONG_FUNCTION);
    let server = start_server(&tree.path().join("endpoint"), LONG_IDLE);
    let arguments = check_arguments(&tree.path().join("long.py"));
    let cli = parsed(&arguments);
    let directory = env::current_dir().expect("hay directorio actual");
    let invocation = Invocation {
        command: &cli.command,
        arguments: &arguments,
        directory: &directory,
        preference: PREFERENCE,
    };
    let Forwarded::Done(finished) =
        forward(&tree.path().join("endpoint"), IDENTITY, &invocation, &mut empty())
    else {
        panic!("el demonio tenía que responder");
    };
    let (report, log, exit) = local_run(&cli, "");
    assert_eq!(finished.report, report);
    assert_eq!(without_timing(&finished.log), without_timing(&log));
    assert_eq!(finished.status, exit);
    let mut delivered_report = Vec::new();
    let mut delivered_log = Vec::new();
    deliver(&finished, &mut delivered_report, &mut delivered_log).expect("se entrega");
    assert_eq!(delivered_report, report.into_bytes());
    assert!(matches!(
        status(&tree.path().join("endpoint")),
        Notice::DaemonRunning { .. }
    ));
    assert_eq!(stop(&tree.path().join("endpoint")), Notice::DaemonStopped);
    server.join().expect("el demonio termina");
    assert_eq!(Endpoint::read(&tree.path().join("endpoint")), None);
    assert_eq!(status(&tree.path().join("endpoint")), Notice::DaemonNotRunning);
    assert_eq!(stop(&tree.path().join("endpoint")), Notice::DaemonNotRunning);
}

#[test]
fn the_hook_gets_its_event_through_the_daemon() {
    let tree = TempTree::new("daemon-hook");
    tree.write("long.py", LONG_FUNCTION);
    let endpoint = tree.path().join("endpoint");
    let server = start_server(&endpoint, LONG_IDLE);
    let arguments: Vec<String> = vec!["kerbo".to_owned(), "hook".to_owned()];
    let cli = parsed(&arguments);
    let event = format!(
        "{{\"tool_input\":{{\"file_path\":{}}}}}",
        serde_json::to_string(&tree.path().join("long.py")).expect("se serializa")
    );
    let directory = env::current_dir().expect("hay directorio actual");
    let invocation = Invocation {
        command: &cli.command,
        arguments: &arguments,
        directory: &directory,
        preference: PREFERENCE,
    };
    let Forwarded::Done(finished) = forward(
        &endpoint,
        IDENTITY,
        &invocation,
        &mut Cursor::new(event.as_bytes()),
    ) else {
        panic!("el demonio tenía que responder");
    };
    let local = local_run(&cli, &event);
    assert_eq!((finished.report, finished.log, finished.status), local);
    stop(&endpoint);
    server.join().expect("el demonio termina");
}

#[test]
fn other_binaries_other_commands_and_wrong_tokens_stay_local() {
    let tree = TempTree::new("daemon-refusals");
    let endpoint = tree.path().join("endpoint");
    let server = start_server(&endpoint, LONG_IDLE);
    let directory = env::current_dir().expect("hay directorio actual");
    let check = check_arguments(&tree.path().join("missing.py"));
    let check_cli = parsed(&check);
    let explain: Vec<String> = ["kerbo", "explain", "duplicate-code"].map(str::to_owned).to_vec();
    let explain_cli = parsed(&explain);
    for (cli, arguments, identity) in [
        (&check_cli, &check, IDENTITY + 1),
        (&explain_cli, &explain, IDENTITY),
    ] {
        let invocation = Invocation {
            command: &cli.command,
            arguments,
            directory: &directory,
            preference: PREFERENCE,
        };
        assert!(matches!(
            forward(&endpoint, identity, &invocation, &mut empty()),
            Forwarded::Local { input: None }
        ));
    }
    let published = Endpoint::read(&endpoint).expect("está publicado");
    let mut stream = TcpStream::connect(published.address()).expect("conecta");
    send(
        &mut stream,
        &Envelope {
            token: "otro".to_owned(),
            request: Request::Stop,
        },
    )
    .expect("se manda");
    assert_eq!(
        receive::<Response>(&mut stream).expect("responde"),
        Response::Refused
    );
    let Some((_, Response::Refused)) = ask(
        &endpoint,
        Request::Run(super::protocol::RunRequest {
            arguments: explain,
            directory,
            flag: None,
            locale: Lang::Spanish,
            input: String::new(),
        }),
    ) else {
        panic!("explain no se corre en el demonio");
    };
    stop(&endpoint);
    server.join().expect("el demonio termina");
}

#[test]
fn an_idle_daemon_stops_by_itself() {
    let tree = TempTree::new("daemon-idle");
    let endpoint = tree.path().join("endpoint");
    start_server(&endpoint, SHORT_IDLE)
        .join()
        .expect("el demonio termina solo");
    assert_eq!(Endpoint::read(&endpoint), None);
}

#[test]
fn start_waits_for_the_daemon_and_reports_when_it_is_already_up_or_cannot_launch() {
    let tree = TempTree::new("daemon-start");
    let endpoint = tree.path().join("endpoint");
    let mut server = None;
    let started = start_with(&endpoint, || {
        server = Some(start_server(&endpoint, LONG_IDLE));
        Ok(())
    });
    assert!(matches!(started, Ok(Notice::DaemonStarted { .. })), "{started:?}");
    let again = start_with(&endpoint, || Ok(()));
    assert!(
        matches!(again, Ok(Notice::DaemonAlreadyRunning { .. })),
        "{again:?}"
    );
    stop(&endpoint);
    server.expect("se lanzó").join().expect("el demonio termina");
    let failed = start_with(&endpoint, || Err(std::io::Error::other("sin binario")));
    assert_eq!(failed, Err(Problem::DaemonUnavailable("sin binario".to_owned())));
}

#[test]
fn the_real_daemon_lives_in_the_user_cache_and_matches_this_binary() {
    assert!(endpoint_directory().is_some());
    let lifetime = default_lifetime();
    assert_eq!(lifetime.idle_timeout, IDLE_TIMEOUT);
    assert_eq!(lifetime.identity, crate::cache::runtime_identity());
}

#[test]
fn the_hook_launches_a_daemon_only_when_none_of_this_binary_answers() {
    let tree = TempTree::new("daemon-ensure");
    let endpoint = tree.path().join("endpoint");
    let mut launches = 0;
    let mut server = None;
    ensure_running_with(&endpoint, IDENTITY, || {
        launches += 1;
        server = Some(start_server(&endpoint, LONG_IDLE));
        Ok(())
    });
    ensure_running_with(&endpoint, IDENTITY, || {
        launches += 1;
        Ok(())
    });
    assert_eq!(launches, 1);
    ensure_running_with(&endpoint, IDENTITY + 1, || {
        launches += 1;
        Ok(())
    });
    assert_eq!(launches, 2);
    server
        .expect("se lanzó")
        .join()
        .expect("el demonio viejo termina");
    assert_eq!(status(&endpoint), Notice::DaemonNotRunning);
}

#[test]
fn a_second_daemon_on_the_same_folder_leaves_at_once() {
    let tree = TempTree::new("daemon-single");
    let endpoint = tree.path().join("endpoint");
    let server = start_server(&endpoint, LONG_IDLE);
    let published = Endpoint::read(&endpoint);
    serve(
        &endpoint,
        Lifetime {
            idle_timeout: LONG_IDLE,
            identity: IDENTITY,
        },
    )
    .expect("el segundo se va sin error");
    assert_eq!(Endpoint::read(&endpoint), published);
    stop(&endpoint);
    server.join().expect("el demonio termina");
}
