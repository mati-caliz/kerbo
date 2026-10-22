pub mod java;
mod java_dereference;
mod java_jdk;
mod java_nullness;
mod java_project;
mod java_resources;
mod java_rules;
mod java_unused;

pub use java_project::IndexedSummary;
pub use java_rules::project_lints;

#[cfg(test)]
mod tests;
