//! Dioxus Fullstack submit adapters for Dioform.
//!
//! The adapter keeps Dioform's submission lifecycle local to the form. Server functions return
//! application-defined transport payloads, and callers map those payloads into structured
//! [`SubmitErrors`]. Transport failures stay on a separate explicit mapping path so network,
//! serialization, or server-function invocation failures do not become submit errors by accident.
//!
//! ```rust,no_run
//! #[cfg(feature = "server")]
//! mod fullstack_submit_example {
//!     extern crate dioxus_server;
//!
//!     use dioxus_core::Event;
//!     use dioform::{Form, FormHandle, SubmitError};
//!     use dioform_fullstack::{ServerSubmitOutcome, SubmitBindingFullstackExt};
//!     use dioxus_fullstack::{server, ServerFnError, ServerFnResult};
//!     use serde::{Deserialize, Serialize};
//!
//!     #[derive(Clone, Debug, Eq, Form, PartialEq)]
//!     struct SignupForm {
//!         email: String,
//!     }
//!
//!     #[derive(Clone, Copy, Debug, Eq, PartialEq)]
//!     enum SignupError {
//!         EmailTaken,
//!         TryAgain,
//!     }
//!
//!     #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
//!     enum SignupRejection {
//!         EmailTaken,
//!     }
//!
//!     #[server]
//!     async fn submit_signup(
//!         email: String,
//!     ) -> ServerFnResult<ServerSubmitOutcome<SignupRejection>> {
//!         if email == "taken@example.com" {
//!             Ok(ServerSubmitOutcome::rejected(SignupRejection::EmailTaken))
//!         } else {
//!             Ok(ServerSubmitOutcome::accepted())
//!         }
//!     }
//!
//!     fn submit_from_form<EventData: ?Sized + 'static>(
//!         form: FormHandle<SignupForm, SignupError>,
//!         event: Event<EventData>,
//!     ) {
//!         form.managed_submit().on_submit_server_fn(
//!             event,
//!             |submitted| async move { submit_signup(submitted.value().email.clone()).await },
//!             |rejection| match rejection {
//!                 SignupRejection::EmailTaken => {
//!                     SubmitError::field(SignupForm::fields().email(), SignupError::EmailTaken).into()
//!                 }
//!             },
//!             |_failure: ServerFnError| SubmitError::form(SignupError::TryAgain).into(),
//!         );
//!     }
//! }
//! ```

use std::future::Future;

use dioform::{IntentSubmitBinding, SubmissionSnapshot, SubmitBinding, SubmitErrors, SubmitResult};
use dioxus_core::Event;

/// Server-side outcome for one submitted form value.
///
/// Transport failures are kept outside this type. A server function can return this as its
/// successful transport payload so application rejections map into structured submit errors while
/// operational failures stay on the explicit failure mapping path.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ServerSubmitOutcome<Rejection> {
    /// The server accepted the submitted value.
    Accepted,
    /// The server rejected the submitted value with application-defined rejection details.
    Rejected(Rejection),
}

impl<Rejection> ServerSubmitOutcome<Rejection> {
    /// Creates an accepted server submit outcome.
    pub const fn accepted() -> Self {
        Self::Accepted
    }

    /// Creates a rejected server submit outcome.
    pub const fn rejected(rejection: Rejection) -> Self {
        Self::Rejected(rejection)
    }
}

impl<Success, Rejection> From<Result<Success, Rejection>> for ServerSubmitOutcome<Rejection> {
    fn from(value: Result<Success, Rejection>) -> Self {
        match value {
            Ok(_) => Self::Accepted,
            Err(rejection) => Self::Rejected(rejection),
        }
    }
}

fn map_server_submit_result<Model, Error, Outcome, Rejection, Failure, MapRejection, MapFailure>(
    result: Result<Outcome, Failure>,
    map_rejection: MapRejection,
    map_failure: MapFailure,
) -> SubmitErrors<Model, Error>
where
    Outcome: Into<ServerSubmitOutcome<Rejection>>,
    MapRejection: FnOnce(Rejection) -> SubmitErrors<Model, Error>,
    MapFailure: FnOnce(Failure) -> SubmitErrors<Model, Error>,
{
    match result {
        Ok(outcome) => match outcome.into() {
            ServerSubmitOutcome::Accepted => SubmitErrors::none(),
            ServerSubmitOutcome::Rejected(rejection) => map_rejection(rejection),
        },
        Err(failure) => map_failure(failure),
    }
}

/// Dioxus Fullstack-oriented submit helpers for ordinary managed submit bindings.
pub trait SubmitBindingFullstackExt<Model, Error> {
    /// Applies a Dioxus `onsubmit` event and starts a managed async submit that calls a
    /// server-function-shaped async operation.
    fn on_submit_server_fn<
        EventData,
        Call,
        Fut,
        Outcome,
        Rejection,
        Failure,
        MapRejection,
        MapFailure,
    >(
        &self,
        event: Event<EventData>,
        call: Call,
        map_rejection: MapRejection,
        map_failure: MapFailure,
    ) -> SubmitResult
    where
        EventData: ?Sized + 'static,
        Call: FnOnce(SubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Result<Outcome, Failure>> + 'static,
        Outcome: Into<ServerSubmitOutcome<Rejection>> + 'static,
        MapRejection: FnOnce(Rejection) -> SubmitErrors<Model, Error> + 'static,
        MapFailure: FnOnce(Failure) -> SubmitErrors<Model, Error> + 'static,
        Model: 'static,
        Error: 'static;
}

impl<Model, Error> SubmitBindingFullstackExt<Model, Error> for SubmitBinding<Model, Error>
where
    Model: Clone + 'static,
    Error: 'static,
{
    fn on_submit_server_fn<
        EventData,
        Call,
        Fut,
        Outcome,
        Rejection,
        Failure,
        MapRejection,
        MapFailure,
    >(
        &self,
        event: Event<EventData>,
        call: Call,
        map_rejection: MapRejection,
        map_failure: MapFailure,
    ) -> SubmitResult
    where
        EventData: ?Sized + 'static,
        Call: FnOnce(SubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Result<Outcome, Failure>> + 'static,
        Outcome: Into<ServerSubmitOutcome<Rejection>> + 'static,
        MapRejection: FnOnce(Rejection) -> SubmitErrors<Model, Error> + 'static,
        MapFailure: FnOnce(Failure) -> SubmitErrors<Model, Error> + 'static,
    {
        self.on_submit_async(event, move |submitted| async move {
            map_server_submit_result(call(submitted).await, map_rejection, map_failure)
        })
    }
}

/// Dioxus Fullstack-oriented submit helpers for intent-scoped managed submit bindings.
pub trait IntentSubmitBindingFullstackExt<Model, Intent, Error> {
    /// Applies a Dioxus `onsubmit` event and starts a managed async submit that calls a
    /// server-function-shaped async operation with the binding's submit intent.
    fn on_submit_server_fn<
        EventData,
        Call,
        Fut,
        Outcome,
        Rejection,
        Failure,
        MapRejection,
        MapFailure,
    >(
        &self,
        event: Event<EventData>,
        call: Call,
        map_rejection: MapRejection,
        map_failure: MapFailure,
    ) -> SubmitResult
    where
        EventData: ?Sized + 'static,
        Call: FnOnce(SubmissionSnapshot<Model, Intent>) -> Fut + 'static,
        Fut: Future<Output = Result<Outcome, Failure>> + 'static,
        Outcome: Into<ServerSubmitOutcome<Rejection>> + 'static,
        MapRejection: FnOnce(Rejection) -> SubmitErrors<Model, Error> + 'static,
        MapFailure: FnOnce(Failure) -> SubmitErrors<Model, Error> + 'static,
        Model: 'static,
        Intent: 'static,
        Error: 'static;
}

impl<Model, Intent, Error> IntentSubmitBindingFullstackExt<Model, Intent, Error>
    for IntentSubmitBinding<Model, Intent, Error>
where
    Model: Clone + 'static,
    Intent: Clone + PartialEq + 'static,
    Error: 'static,
{
    fn on_submit_server_fn<
        EventData,
        Call,
        Fut,
        Outcome,
        Rejection,
        Failure,
        MapRejection,
        MapFailure,
    >(
        &self,
        event: Event<EventData>,
        call: Call,
        map_rejection: MapRejection,
        map_failure: MapFailure,
    ) -> SubmitResult
    where
        EventData: ?Sized + 'static,
        Call: FnOnce(SubmissionSnapshot<Model, Intent>) -> Fut + 'static,
        Fut: Future<Output = Result<Outcome, Failure>> + 'static,
        Outcome: Into<ServerSubmitOutcome<Rejection>> + 'static,
        MapRejection: FnOnce(Rejection) -> SubmitErrors<Model, Error> + 'static,
        MapFailure: FnOnce(Failure) -> SubmitErrors<Model, Error> + 'static,
    {
        self.on_submit_async(event, move |submitted| async move {
            map_server_submit_result(call(submitted).await, map_rejection, map_failure)
        })
    }
}
