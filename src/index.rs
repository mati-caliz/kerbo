pub mod java;
mod java_dereference;
mod java_jdk;
mod java_nullness;
mod java_project;
mod java_resources;
mod java_rules;
mod java_taint;
mod java_unused;
mod taint_engine;
mod taint_reach;

pub use java_project::IndexedSummary;
pub use java_rules::project_lints;

#[cfg(test)]
mod taint_tests;
#[cfg(test)]
mod tests;
