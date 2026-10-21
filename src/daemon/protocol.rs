use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;

const MAX_MESSAGE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub token: String,
    pub request: Request,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    Run(RunRequest),
    Status,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRequest {
    pub arguments: Vec<String>,
    pub directory: PathBuf,
    pub flag: Option<Lang>,
    pub locale: Lang,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    Finished(Finished),
    Running { pid: u32 },
    Stopping,
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finished {
    pub report: String,
    pub log: String,
    pub status: u8,
}

pub fn send<Message: Serialize>(stream: &mut TcpStream, message: &Message) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, message)?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)
}

pub fn receive<Message: DeserializeOwned>(stream: &mut TcpStream) -> io::Result<Message> {
    let mut bytes = Vec::new();
    stream.take(MAX_MESSAGE_BYTES).read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::from)
}
