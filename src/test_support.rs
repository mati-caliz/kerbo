use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

static CREATED: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct TempTree {
    root: PathBuf,
}

impl TempTree {
    pub fn new(name: &str) -> Self {
        let sequence = CREATED.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("kerbo-test-{}-{sequence}-{name}", process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("se borra el directorio de una corrida anterior");
        }
        fs::create_dir_all(&root).expect("se crea el directorio temporal");
        Self { root }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("se crea la carpeta del archivo");
        }
        fs::write(path, contents).expect("se escribe el archivo");
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.root);
    }
}
