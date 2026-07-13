//! Realistic, multi-field product forms. Unlike the feature examples, these are
//! full pages that combine several features at once and are not source-quoted.

mod checkout;
mod invoice;
mod presentation;
mod project_planner;
mod signup;
mod support;

use presentation::{field_checkbox, field_select, field_text};

pub use checkout::Checkout;
pub use invoice::Invoice;
pub use project_planner::ProjectPlanner;
pub use signup::Signup;
