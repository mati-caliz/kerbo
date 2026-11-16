pub mod ecmascript_taint;
pub mod go_taint;
pub mod java;
pub mod java_annotation_values;
mod java_default_annotation_params;
mod java_dereference;
mod java_escapes;
mod java_exact_classes;
pub mod java_exposure;
mod java_failing_calls;
mod java_forwarded_returns;
mod java_header;
mod java_import_shadow_rule;
mod java_inferred_arguments;
mod java_inherited_returns;
mod java_inspection_suppressions;
mod java_jdk;
mod java_lambda_null_returns;
mod java_library_arguments;
mod java_member_types;
mod java_null_arguments;
mod java_null_origins;
mod java_nullable_returns;
mod java_nullness;
mod java_parameter_nullness;
mod java_project;
mod java_reference_unboxing;
mod java_resource_owners;
mod java_resources;
mod java_rules;
mod java_sequenced_types;
mod java_size_chain_rule;
mod java_substituted_returns;
mod java_taint;
mod java_throw_functional;
mod java_throw_lambdas;
mod java_throw_lookup;
mod java_throw_project;
mod java_throw_roots;
mod java_throw_wanted;
mod java_throws;
mod java_typed;
mod java_unused;
mod java_variable_returns;
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
mod java_default_annotation_param_tests;
#[cfg(test)]
mod java_deserialization_tests;
#[cfg(test)]
mod java_escapes_tests;
#[cfg(test)]
mod java_failing_calls_tests;
#[cfg(test)]
mod java_import_shadow_tests;
#[cfg(test)]
mod java_member_type_tests;
#[cfg(test)]
mod java_null_assertion_tests;
#[cfg(test)]
mod java_null_iteration_tests;
#[cfg(test)]
mod java_null_lambda_return_tests;
#[cfg(test)]
mod java_null_library_argument_tests;
#[cfg(test)]
mod java_null_marked_tests;
#[cfg(test)]
mod java_null_parameter_tests;
#[cfg(test)]
mod java_null_reference_tests;
#[cfg(test)]
mod java_null_reread_tests;
#[cfg(test)]
mod java_null_tests;
#[cfg(test)]
mod java_null_variable_return_tests;
#[cfg(test)]
mod java_nullable_field_tests;
#[cfg(test)]
mod java_nullable_return_tests;
#[cfg(test)]
mod java_resource_tests;
#[cfg(test)]
mod java_sequenced_types_tests;
#[cfg(test)]
mod java_size_chain_tests;
#[cfg(test)]
mod java_throw_abstract_tests;
#[cfg(test)]
mod java_throw_ambiguous_tests;
#[cfg(test)]
mod java_throw_functional_tests;
#[cfg(test)]
mod java_throw_reach_tests;
#[cfg(test)]
mod java_throws_tests;
#[cfg(test)]
mod java_typed_tests;
#[cfg(test)]
mod python_taint_tests;
#[cfg(test)]
mod taint_tests;
#[cfg(test)]
mod tests;
