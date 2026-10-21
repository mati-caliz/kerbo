use std::collections::hash_map::RandomState;
use std::fs::{self, OpenOptions};
use std::hash::BuildHasher;
use std::io::{self, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

const ENDPOINT_FILE: &str = "daemon.json";
const TEMPORARY_EXTENSION: &str = "tmp";
#[cfg(unix)]
const OWNER_ONLY: u32 = 0o600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    pub port: u16,
    pub token: String,
    pub identity: u64,
    pub pid: u32,
}

impl Endpoint {
    pub fn new(port: u16, identity: u64) -> Self {
        Self {
            port,
            token: new_token(),
            identity,
            pid: process::id(),
        }
    }

    pub fn address(&self) -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, self.port))
    }

    pub fn publish(&self, directory: &Path) -> io::Result<()> {
        fs::create_dir_all(directory)?;
        let path = endpoint_path(directory);
        let temporary = path.with_extension(format!("{}.{TEMPORARY_EXTENSION}", process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        restrict_to_owner(&mut options);
        let mut file = options.open(&temporary)?;
        file.write_all(&serde_json::to_vec(self)?)?;
        drop(file);
        fs::rename(&temporary, &path)
    }

    pub fn read(directory: &Path) -> Option<Self> {
        let bytes = fs::read(endpoint_path(directory)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn withdraw(&self, directory: &Path) {
        if Self::read(directory).is_some_and(|published| published.token == self.token) {
            let _removed = fs::remove_file(endpoint_path(directory));
        }
    }
}

fn endpoint_path(directory: &Path) -> PathBuf {
    directory.join(ENDPOINT_FILE)
}

fn new_token() -> String {
    let first = RandomState::new().hash_one(process::id());
    let second = RandomState::new().hash_one(SystemTime::now());
    format!("{first:016x}{second:016x}")
}

#[cfg(unix)]
fn restrict_to_owner(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(OWNER_ONLY);
}

#[cfg(not(unix))]
const fn restrict_to_owner(_options: &mut OpenOptions) {}
