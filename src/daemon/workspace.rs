use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, PoisonError};

#[derive(Debug, Default)]
pub struct Workspace {
    occupancy: Mutex<Occupancy>,
    released: Condvar,
}

#[derive(Debug, Default)]
struct Occupancy {
    directory: Option<PathBuf>,
    active: usize,
}

#[derive(Debug)]
pub struct Visit<'workspace> {
    workspace: &'workspace Workspace,
}

impl Workspace {
    pub fn enter(&self, directory: &Path) -> io::Result<Visit<'_>> {
        let guard = self.occupancy.lock().unwrap_or_else(PoisonError::into_inner);
        let mut occupancy = self
            .released
            .wait_while(guard, |occupancy| {
                occupancy.active > 0 && occupancy.directory.as_deref() != Some(directory)
            })
            .unwrap_or_else(PoisonError::into_inner);
        if occupancy.directory.as_deref() != Some(directory) {
            env::set_current_dir(directory)?;
            occupancy.directory = Some(directory.to_path_buf());
        }
        occupancy.active = occupancy.active.saturating_add(1);
        drop(occupancy);
        Ok(Visit { workspace: self })
    }
}

impl Drop for Visit<'_> {
    fn drop(&mut self) {
        let mut occupancy = self
            .workspace
            .occupancy
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        occupancy.active = occupancy.active.saturating_sub(1);
        drop(occupancy);
        self.workspace.released.notify_all();
    }
}

#[cfg(test)]
mod tests;
