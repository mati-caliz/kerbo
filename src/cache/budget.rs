use std::env;
use std::ffi::OsString;

const MAX_MEGABYTES_VARIABLE: &str = "KERBO_CACHE_MAX_MB";
const BYTES_PER_MEGABYTE: usize = 1024 * 1024;
const DEFAULT_MAX_MEGABYTES: usize = 256;
pub const DEFAULT_MAX_BYTES: usize = DEFAULT_MAX_MEGABYTES * BYTES_PER_MEGABYTE;

pub fn max_bytes(configured_megabytes: Option<usize>) -> usize {
    resolve(env::var_os(MAX_MEGABYTES_VARIABLE), configured_megabytes)
}

pub fn resolve(variable: Option<OsString>, configured_megabytes: Option<usize>) -> usize {
    variable
        .and_then(|value| value.to_str()?.trim().parse::<usize>().ok())
        .or(configured_megabytes)
        .map_or(DEFAULT_MAX_BYTES, |megabytes| {
            megabytes.saturating_mul(BYTES_PER_MEGABYTE)
        })
}
