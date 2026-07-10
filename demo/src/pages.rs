//! Route components. Each page frames one or more `examples` components with
//! prose, a docs link, and the example's own source (via `code!`).

mod basics;
mod fields;
mod forms;
mod server;
mod submission;
mod validation;

pub use basics::{FieldBindings, Home, Minimal, ParsedInputs};
pub use fields::{
    CollectionValidation, Collections, FieldGroups, FileFields, NestedPaths, Observers,
    Serialization, StateMeta,
};
pub use forms::{Checkout, Invoice, ProjectPlanner, Signup};
pub use server::ServerValidation;
pub use submission::{BrowserSubmission, SubmitErrors, SubmitIntents};
pub use validation::{
    AdapterValidation, AsyncValidation, ErrorSummary, ValidationModes, Validators,
};
