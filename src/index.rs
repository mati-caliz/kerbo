pub mod ecmascript_taint;
pub mod go_taint;
pub mod java;
mod java_dereference;
mod java_escapes;
pub mod java_exposure;
mod java_jdk;
mod java_nullness;
mod java_project;
mod java_resources;
mod java_rules;
mod java_taint;
mod java_unused;
pub mod python_taint;
mod taint_engine;
mod taint_reach;
pub mod templates;

pub use java_project::IndexedSummary;
pub use java_rules::project_lints;

#[cfg(test)]
mod ecmascript_taint_tests;
#[cfg(test)]
mod go_taint_tests;
#[cfg(test)]
mod java_escapes_tests;
#[cfg(test)]
mod python_taint_tests;
#[cfg(test)]
mod taint_tests;
#[cfg(test)]
mod tests;
