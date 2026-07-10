//! Shared support for Dioform **Validation Adapters**.
//!
//! A **Validation Adapter** maps an external validation library's diagnostics into the form's shared
//! **Validation Error** type. Every adapter needs the same two pieces of plumbing: a map from an
//! **External Diagnostic Path** to a typed **Validation Target**, and a borrowed view of one external
//! diagnostic paired with the target it resolved to. This crate owns both so each first-party adapter
//! (`dioform-garde`, `dioform-validator`, and any future adapter) does not re-derive them.
//!
//! What stays in each adapter is the part that genuinely differs: how the external library enumerates
//! its diagnostics, and the builder whose `register` bounds name that library's validation trait. The
//! field-versus-form routing lives in the **Form Core** as
//! [`FormValidationError::for_target`](dioform_core::FormValidationError::for_target); this crate
//! only bridges an **External Diagnostic Path** to the [`ValidationTarget`] that constructor consumes.

use std::{collections::BTreeMap, fmt, marker::PhantomData};

use dioform_core::{FieldPath, ValidationTarget};

/// A map from an **External Diagnostic Path** (the string an external validation library emits) to a
/// typed **Validation Target** in one **Form Model**.
///
/// Registered paths attach to their typed field targets; unregistered paths resolve to the form, so an
/// unknown diagnostic is preserved as a form-level error rather than dropped or matched by field name.
pub struct PathMap<Model> {
    targets: BTreeMap<String, ValidationTarget>,
    _marker: PhantomData<fn() -> Model>,
}

impl<Model> Clone for PathMap<Model> {
    fn clone(&self) -> Self {
        Self {
            targets: self.targets.clone(),
            _marker: PhantomData,
        }
    }
}

impl<Model> Default for PathMap<Model> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Model> PathMap<Model> {
    /// Creates an empty path map. All diagnostics resolve to the form until fields are registered.
    pub fn new() -> Self {
        Self {
            targets: BTreeMap::new(),
            _marker: PhantomData,
        }
    }

    /// Returns a new path map with one exact external path registered to a typed field path.
    pub fn with_field<Value>(
        mut self,
        external_path: impl Into<String>,
        field: FieldPath<Model, Value>,
    ) -> Self {
        self.insert_field(external_path, field);
        self
    }

    /// Registers one exact external path to a typed field path.
    ///
    /// If the path was already mapped, returns the previous target.
    pub fn insert_field<Value>(
        &mut self,
        external_path: impl Into<String>,
        field: FieldPath<Model, Value>,
    ) -> Option<ValidationTarget> {
        self.targets
            .insert(external_path.into(), ValidationTarget::field(field))
    }

    /// Resolves an exact external path string into a Dioform validation target.
    pub fn target_for_path(&self, external_path: &str) -> ValidationTarget {
        self.targets
            .get(external_path)
            .cloned()
            .unwrap_or_else(ValidationTarget::form)
    }
}

/// A borrowed view of one **External Validation Diagnostic** paired with the **Validation Target** it
/// resolved to.
///
/// This is the value an adapter hands to a mapper closure so the application can inspect the original
/// external path and error, and the chosen target, before mapping the diagnostic into the shared
/// **Validation Error** type. `Path` and `Err` are the external library's own types, borrowed for the
/// duration of one mapper call; both are `?Sized` so an adapter can view a `str` path directly.
pub struct DiagnosticView<'a, Path: ?Sized, Err: ?Sized> {
    path: &'a Path,
    error: &'a Err,
    target: ValidationTarget,
}

impl<'a, Path: ?Sized, Err: ?Sized> DiagnosticView<'a, Path, Err> {
    /// Pairs a borrowed external diagnostic with the target it resolved to.
    pub const fn new(path: &'a Path, error: &'a Err, target: ValidationTarget) -> Self {
        Self {
            path,
            error,
            target,
        }
    }

    /// Returns the original external diagnostic path.
    pub const fn path(&self) -> &'a Path {
        self.path
    }

    /// Returns the original external diagnostic error.
    pub const fn error(&self) -> &'a Err {
        self.error
    }

    /// Returns the Dioform target selected for this diagnostic.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }
}

impl<Path: ?Sized, Err: ?Sized> Clone for DiagnosticView<'_, Path, Err> {
    fn clone(&self) -> Self {
        Self {
            path: self.path,
            error: self.error,
            target: self.target.clone(),
        }
    }
}

impl<Path: ?Sized + fmt::Debug, Err: ?Sized + fmt::Debug> fmt::Debug
    for DiagnosticView<'_, Path, Err>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiagnosticView")
            .field("path", &self.path)
            .field("error", &self.error)
            .field("target", &self.target)
            .finish()
    }
}
