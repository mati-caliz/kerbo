use std::env;
use std::path::PathBuf;

const DIRECTORY_VARIABLE: &str = "KERBO_CACHE_DIR";
const APPLICATION_DIRECTORY: &str = "kerbo";

pub fn default_directory() -> Option<PathBuf> {
    env::var_os(DIRECTORY_VARIABLE)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| platform_cache_root().map(|root| root.join(APPLICATION_DIRECTORY)))
}

fn absolute_variable(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

#[cfg(windows)]
fn platform_cache_root() -> Option<PathBuf> {
    const LOCAL_DATA_VARIABLE: &str = "LOCALAPPDATA";
    absolute_variable(LOCAL_DATA_VARIABLE)
}

#[cfg(target_os = "macos")]
fn platform_cache_root() -> Option<PathBuf> {
    const HOME_VARIABLE: &str = "HOME";
    const CACHES_DIRECTORY: &str = "Library/Caches";
    absolute_variable(HOME_VARIABLE).map(|home| home.join(CACHES_DIRECTORY))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_cache_root() -> Option<PathBuf> {
    const XDG_VARIABLE: &str = "XDG_CACHE_HOME";
    const HOME_VARIABLE: &str = "HOME";
    const CACHE_DIRECTORY: &str = ".cache";
    absolute_variable(XDG_VARIABLE)
        .or_else(|| absolute_variable(HOME_VARIABLE).map(|home| home.join(CACHE_DIRECTORY)))
}
