//! Small, focused example components: one per feature area.
//!
//! Each component keeps the `dioform` API front and center; project-agnostic
//! presentation lives in [`crate::components`]. The `pages` module both mounts
//! these live and renders their source with the compile-time `code!` macro.

pub mod adapter_validation;
pub mod async_validation;
pub mod browser_submission;
pub mod collection_validation;
pub mod collections;
pub mod error_summary;
pub mod field_bindings;
pub mod field_groups;
pub mod file_fields;
pub mod minimal;
pub mod nested_paths;
pub mod observers;
pub mod parsed_inputs;
mod presentation;
pub mod serialization;
pub mod server_validation;
pub mod state_meta;
pub mod submit_errors;
pub mod submit_intents;
pub mod validation_modes;
pub mod validators;

pub(crate) use presentation::StateGrid;
