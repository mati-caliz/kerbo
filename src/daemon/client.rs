use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use super::endpoint::Endpoint;
use super::protocol::{Envelope, Finished, Request, Response, RunRequest, receive, send};
use crate::cli::Command;
use crate::i18n::LangPreference;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Debug)]
pub struct Invocation<'invocation> {
    pub command: &'invocation Command,
    pub arguments: &'invocation [String],
    pub directory: &'invocation Path,
    pub preference: LangPreference,
}

#[derive(Debug)]
pub enum Forwarded {
    Done(Finished),
    Local { input: Option<String> },
}

pub fn forward(
    endpoint_directory: &Path,
    identity: u64,
    invocation: &Invocation<'_>,
    input: &mut dyn Read,
) -> Forwarded {
    let reads_input = matches!(invocation.command, Command::Hook(_));
    if !matches!(invocation.command, Command::Check(_) | Command::Hook(_)) {
        return Forwarded::Local { input: None };
    }
    let Some(endpoint) = Endpoint::read(endpoint_directory).filter(|endpoint| endpoint.identity == identity)
    else {
        return Forwarded::Local { input: None };
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&endpoint.address(), CONNECT_TIMEOUT) else {
        return Forwarded::Local { input: None };
    };
    let mut text = String::new();
    if reads_input && input.read_to_string(&mut text).is_err() {
        return Forwarded::Local { input: Some(text) };
    }
    let envelope = Envelope {
        token: endpoint.token,
        request: Request::Run(RunRequest {
            arguments: invocation.arguments.to_vec(),
            directory: invocation.directory.to_path_buf(),
            flag: invocation.preference.flag,
            locale: invocation.preference.locale,
            input: text.clone(),
        }),
    };
    match exchange(&mut stream, &envelope) {
        Some(Response::Finished(finished)) => Forwarded::Done(finished),
        Some(Response::Running { .. } | Response::Stopping | Response::Refused) | None => Forwarded::Local {
            input: reads_input.then_some(text),
        },
    }
}

pub fn ask(endpoint_directory: &Path, request: Request) -> Option<(Endpoint, Response)> {
    let endpoint = Endpoint::read(endpoint_directory)?;
    let mut stream = TcpStream::connect_timeout(&endpoint.address(), CONNECT_TIMEOUT).ok()?;
    let envelope = Envelope {
        token: endpoint.token.clone(),
        request,
    };
    let response = exchange(&mut stream, &envelope)?;
    Some((endpoint, response))
}

pub fn deliver(finished: &Finished, report: &mut dyn Write, log: &mut dyn Write) -> io::Result<()> {
    report.write_all(finished.report.as_bytes())?;
    report.flush()?;
    log.write_all(finished.log.as_bytes())?;
    log.flush()
}

fn exchange(stream: &mut TcpStream, envelope: &Envelope) -> Option<Response> {
    send(stream, envelope).ok()?;
    receive(stream).ok()
}
