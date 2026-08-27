//! Route components grouped by navigation section. Focused feature pages frame
//! live `examples` with prose and source; realistic forms compose features into
//! complete product pages without quoting their full source.

mod basics;
mod fields;
mod forms;
mod not_found;
mod server;
mod submission;
mod validation;

pub use basics::{DioxusFieldRegistry, FieldBindings, HappyPath, Home, Minimal, ParsedInputs};
pub use fields::{
    CollectionValidation, Collections, FieldGroups, FileFields, NestedPaths, Observers,
    OptionalFields, Serialization, StateMeta,
};
pub use forms::{Checkout, Invoice, ProjectPlanner, Signup};
pub use not_found::NotFound;
pub use server::ServerValidation;
pub use submission::{BrowserSubmission, SubmitErrors, SubmitIntents};
pub use validation::{
    AdapterValidation, AsyncValidation, ErrorSummary, ValidationModes, Validators,
};
