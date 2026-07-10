//! Renderer-agnostic form state for Dioform.
//!
//! The core owns form drafts, typed field paths, validation state, submission state, reset and
//! reinitialization semantics, and value-redacted observer events without depending on Dioxus or a
//! concrete async runtime.
//!
//! Async and debounced validation cross the runtime boundary through explicit work-token APIs. The
//! core decides when a validator is pending, skipped, stale, valid, or invalid; adapters execute the
//! returned work from owned [`FormSnapshot`] values and complete it back into the core.

use std::{
    any::Any,
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt,
    marker::PhantomData,
    rc::Rc,
};

/// A form model that can expose generated typed field paths.
pub trait Form {
    /// The generated field accessor namespace for this model.
    type Fields;

    /// Returns the generated direct field accessor namespace.
    fn fields() -> Self::Fields;
}

/// A reusable typed group of fields that can be mounted into a form model.
pub trait FieldGroup: Sized {
    /// The generated field group map for a host form model.
    type Map<Model>;

    /// Mounts this field group under a typed parent path in a host form model.
    fn mount<Model>(prefix: FieldPath<Model, Self>) -> Self::Map<Model>
    where
        Model: 'static,
        Self: 'static;
}

/// Opaque internal identity for one logical item inside a collection field.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CollectionItemIdentity(u64);

impl CollectionItemIdentity {
    pub(crate) const fn as_u64(self) -> u64 {
        self.0
    }

    /// Returns a stable opaque key suitable for rendering and diagnostics.
    pub fn key(self) -> String {
        format!("item-{}", self.0)
    }
}

impl fmt::Display for CollectionItemIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "item-{}", self.0)
    }
}

/// One logical item in the current rendered order of a collection field.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectionItem {
    identity: CollectionItemIdentity,
    index: usize,
}

impl CollectionItem {
    /// Returns the internal logical item identity.
    pub const fn identity(self) -> CollectionItemIdentity {
        self.identity
    }

    /// Returns the item's current rendered index.
    pub const fn index(self) -> usize {
        self.index
    }
}

/// Internal identity for a field, separate from the rendered field name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FieldIdentity {
    kind: FieldIdentityKind,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum FieldIdentityKind {
    /// A statically known field path generated from a named form struct.
    Static { path: Rc<str> },
    /// A form-scoped file selection outside the form model.
    File { name: Rc<str> },
    /// A statically known child field inside one logical item of a direct collection field.
    CollectionItem {
        collection: Rc<str>,
        item: CollectionItemIdentity,
        field: Rc<str>,
    },
}

fn owned_segment(segment: impl Into<Rc<str>>) -> Rc<str> {
    segment.into()
}

#[cfg(feature = "serde")]
impl serde::Serialize for FieldIdentity {
    fn serialize<Serializer>(
        &self,
        serializer: Serializer,
    ) -> Result<Serializer::Ok, Serializer::Error>
    where
        Serializer: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum SerializableFieldIdentity<'a> {
            Static {
                path: &'a str,
            },
            File {
                name: &'a str,
            },
            CollectionItem {
                collection: &'a str,
                item: CollectionItemIdentity,
                field: &'a str,
            },
        }

        match &self.kind {
            FieldIdentityKind::Static { path } => SerializableFieldIdentity::Static {
                path: path.as_ref(),
            },
            FieldIdentityKind::File { name } => SerializableFieldIdentity::File {
                name: name.as_ref(),
            },
            FieldIdentityKind::CollectionItem {
                collection,
                item,
                field,
            } => SerializableFieldIdentity::CollectionItem {
                collection: collection.as_ref(),
                item: *item,
                field: field.as_ref(),
            },
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for FieldIdentity {
    fn deserialize<Deserializer>(deserializer: Deserializer) -> Result<Self, Deserializer::Error>
    where
        Deserializer: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum SerializableFieldIdentity {
            Static {
                path: String,
            },
            File {
                name: String,
            },
            CollectionItem {
                collection: String,
                item: CollectionItemIdentity,
                field: String,
            },
        }

        Ok(
            match SerializableFieldIdentity::deserialize(deserializer)? {
                SerializableFieldIdentity::Static { path } => Self::new(path),
                SerializableFieldIdentity::File { name } => Self::file(name),
                SerializableFieldIdentity::CollectionItem {
                    collection,
                    item,
                    field,
                } => Self::collection_item(collection, item, field),
            },
        )
    }
}

impl FieldIdentity {
    /// Creates a static field identity.
    pub fn new(path: impl Into<Rc<str>>) -> Self {
        Self {
            kind: FieldIdentityKind::Static {
                path: owned_segment(path),
            },
        }
    }

    /// Creates a file-selection identity outside the form model.
    pub fn file(name: impl Into<Rc<str>>) -> Self {
        Self {
            kind: FieldIdentityKind::File {
                name: owned_segment(name),
            },
        }
    }

    /// Creates an item-child field identity for the first direct collection-field slice.
    pub fn collection_item(
        collection: impl Into<Rc<str>>,
        item: CollectionItemIdentity,
        field: impl Into<Rc<str>>,
    ) -> Self {
        Self {
            kind: FieldIdentityKind::CollectionItem {
                collection: owned_segment(collection),
                item,
                field: owned_segment(field),
            },
        }
    }

    /// Creates an identity for a collection item value itself.
    ///
    /// This is used by helpers such as true multi-select fields where the selected value is the
    /// collection item, rather than a child field inside a row struct.
    pub fn collection_item_value(
        collection: impl Into<Rc<str>>,
        item: CollectionItemIdentity,
    ) -> Self {
        Self {
            kind: FieldIdentityKind::CollectionItem {
                collection: owned_segment(collection),
                item,
                field: owned_segment(""),
            },
        }
    }

    /// Returns the static identity path for static fields, the static child-field segment for a
    /// collection item child field, or an empty segment for a collection item value identity.
    pub fn as_str(&self) -> &str {
        match &self.kind {
            FieldIdentityKind::Static { path } => path.as_ref(),
            FieldIdentityKind::File { name } => name.as_ref(),
            FieldIdentityKind::CollectionItem { field, .. } => field.as_ref(),
        }
    }

    /// Returns the static field path for statically known fields.
    pub fn static_path(&self) -> Option<&str> {
        match &self.kind {
            FieldIdentityKind::Static { path } => Some(path.as_ref()),
            FieldIdentityKind::File { .. } => None,
            FieldIdentityKind::CollectionItem { .. } => None,
        }
    }

    /// Returns whether this identity addresses a file selection outside the form model.
    pub fn is_file(&self) -> bool {
        matches!(&self.kind, FieldIdentityKind::File { .. })
    }

    /// Returns the parent collection path for collection item identities.
    pub fn collection_path(&self) -> Option<&str> {
        match &self.kind {
            FieldIdentityKind::CollectionItem { collection, .. } => Some(collection.as_ref()),
            FieldIdentityKind::Static { .. } | FieldIdentityKind::File { .. } => None,
        }
    }

    /// Returns the logical item identity for collection item identities.
    pub fn collection_item_identity(&self) -> Option<CollectionItemIdentity> {
        match &self.kind {
            FieldIdentityKind::CollectionItem { item, .. } => Some(*item),
            FieldIdentityKind::Static { .. } | FieldIdentityKind::File { .. } => None,
        }
    }

    /// Returns whether this identity addresses a collection item value rather than a child field.
    pub fn is_collection_item_value(&self) -> bool {
        matches!(&self.kind, FieldIdentityKind::CollectionItem { field, .. } if field.is_empty())
    }

    fn as_static_path(&self) -> Option<&str> {
        self.static_path()
    }

    fn collection_item_parts(&self) -> Option<(&str, CollectionItemIdentity, &str)> {
        match &self.kind {
            FieldIdentityKind::CollectionItem {
                collection,
                item,
                field,
            } => Some((collection.as_ref(), *item, field.as_ref())),
            FieldIdentityKind::Static { .. } | FieldIdentityKind::File { .. } => None,
        }
    }

    fn is_static(&self) -> bool {
        matches!(self.kind, FieldIdentityKind::Static { .. })
    }

    fn is_collection_item_for(&self, collection: &str, item: CollectionItemIdentity) -> bool {
        matches!(
            &self.kind,
            FieldIdentityKind::CollectionItem {
                collection: candidate_collection,
                item: candidate_item,
                ..
            } if candidate_collection.as_ref() == collection && *candidate_item == item
        )
    }
}

type FieldPathGet<Model, Value> = dyn for<'a> Fn(&'a Model) -> &'a Value + 'static;
type FieldPathGetMut<Model, Value> = dyn for<'a> Fn(&'a mut Model) -> &'a mut Value + 'static;

struct FieldPathAccessor<Model, Value> {
    get: Rc<FieldPathGet<Model, Value>>,
    get_mut: Rc<FieldPathGetMut<Model, Value>>,
}

impl<Model, Value> Clone for FieldPathAccessor<Model, Value> {
    fn clone(&self) -> Self {
        Self {
            get: Rc::clone(&self.get),
            get_mut: Rc::clone(&self.get_mut),
        }
    }
}

impl<Model, Value> FieldPathAccessor<Model, Value> {
    fn direct(
        get: for<'a> fn(&'a Model) -> &'a Value,
        get_mut: for<'a> fn(&'a mut Model) -> &'a mut Value,
    ) -> Self
    where
        Model: 'static,
        Value: 'static,
    {
        Self {
            get: Rc::new(get),
            get_mut: Rc::new(get_mut),
        }
    }

    fn get<'a>(&self, model: &'a Model) -> &'a Value {
        (self.get)(model)
    }

    fn get_mut<'a>(&self, model: &'a mut Model) -> &'a mut Value {
        (self.get_mut)(model)
    }
}

fn join_static_path(parent: &str, child: &str) -> Rc<str> {
    match (parent.is_empty(), child.is_empty()) {
        (true, true) => owned_segment(""),
        (true, false) => owned_segment(child),
        (false, true) => owned_segment(parent),
        (false, false) => owned_segment(format!("{parent}.{child}")),
    }
}

/// A typed path from a form model to one field value.
pub struct FieldPath<Model, Value> {
    identity: FieldIdentity,
    field_name: Rc<str>,
    accessor: FieldPathAccessor<Model, Value>,
    _marker: PhantomData<fn() -> (Model, Value)>,
}

impl<Model, Value> Clone for FieldPath<Model, Value> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            field_name: Rc::clone(&self.field_name),
            accessor: self.accessor.clone(),
            _marker: PhantomData,
        }
    }
}

impl<Model, Value> fmt::Debug for FieldPath<Model, Value> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FieldPath")
            .field("identity", &self.identity)
            .field("field_name", &self.field_name)
            .finish_non_exhaustive()
    }
}

impl<Model, Value> FieldPath<Model, Value> {
    /// Creates a typed direct field path.
    pub fn direct(
        identity: FieldIdentity,
        field_name: &'static str,
        get: for<'a> fn(&'a Model) -> &'a Value,
        get_mut: for<'a> fn(&'a mut Model) -> &'a mut Value,
    ) -> Self
    where
        Model: 'static,
        Value: 'static,
    {
        Self {
            identity,
            field_name: Rc::from(field_name),
            accessor: FieldPathAccessor::direct(get, get_mut),
            _marker: PhantomData,
        }
    }

    /// Composes this path with a child path from its value type.
    ///
    /// The resulting path stays typed from the original form model to the nested value. Its
    /// **Field Identity** and rendered **Field Name** use dot-separated static path segments, so a
    /// nested collection path such as `invoice.lines` can still be passed to collection APIs.
    ///
    /// Joined paths are interned for the lifetime of the process so composed paths remain cheap,
    /// copyable values with static rendered names.
    pub fn try_join<Nested>(
        self,
        child: FieldPath<Value, Nested>,
    ) -> Option<FieldPath<Model, Nested>>
    where
        Model: 'static,
        Value: 'static,
        Nested: 'static,
    {
        let parent_identity = self.identity.as_static_path()?.to_owned();
        let child_identity = child.identity.as_static_path()?.to_owned();
        let identity = FieldIdentity::new(join_static_path(&parent_identity, &child_identity));
        let field_name = join_static_path(&self.field_name, &child.field_name);
        let parent_for_get = self.clone();
        let child_for_get = child.clone();
        let parent_for_get_mut = self;
        let child_for_get_mut = child;

        Some(FieldPath {
            identity,
            field_name,
            accessor: FieldPathAccessor {
                get: Rc::new(move |model| child_for_get.get(parent_for_get.get(model))),
                get_mut: Rc::new(move |model| {
                    child_for_get_mut.get_mut(parent_for_get_mut.get_mut(model))
                }),
            },
            _marker: PhantomData,
        })
    }

    /// Composes two static field paths, panicking if either side is not a static path.
    ///
    /// Use [`FieldPath::try_join`] when composing paths from values that may represent collection
    /// item identities rather than named-struct field paths.
    pub fn join<Nested>(self, child: FieldPath<Value, Nested>) -> FieldPath<Model, Nested>
    where
        Model: 'static,
        Value: 'static,
        Nested: 'static,
    {
        self.try_join(child)
            .expect("nested field paths require static parent and child field identities")
    }

    /// Returns the internal field identity.
    pub fn identity(&self) -> FieldIdentity {
        self.identity.clone()
    }

    /// Returns the rendered field name for HTML interoperability.
    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    /// Reads this field from a model value.
    pub fn get<'a>(&self, model: &'a Model) -> &'a Value {
        self.accessor.get(model)
    }

    /// Mutably reads this field from a model value.
    pub fn get_mut<'a>(&self, model: &'a mut Model) -> &'a mut Value {
        self.accessor.get_mut(model)
    }
}

/// Stable identity for one registered validator.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValidatorId(u64);

impl ValidatorId {
    /// Returns the stable registration-order identifier for this validator.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ValidatorId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Human-readable source label for one registered validator or submit error.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValidatorSource {
    name: String,
}

impl ValidatorSource {
    /// Creates a validator source label.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the source label.
    pub fn as_str(&self) -> &str {
        &self.name
    }

    /// Creates the default debug label used when callers do not need a custom one.
    pub fn anonymous() -> Self {
        Self::new("validator")
    }

    /// Creates the default source used for application submit errors.
    pub fn submit() -> Self {
        Self::new("submit")
    }
}

impl AsRef<str> for ValidatorSource {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ValidatorSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&str> for ValidatorSource {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ValidatorSource {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<Cow<'_, str>> for ValidatorSource {
    fn from(value: Cow<'_, str>) -> Self {
        Self::new(value.into_owned())
    }
}

impl From<ValidatorSource> for String {
    fn from(value: ValidatorSource) -> Self {
        value.name
    }
}

impl From<()> for ValidatorSource {
    fn from(_value: ()) -> Self {
        Self::anonymous()
    }
}

/// Semantic event that caused a validation run.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ValidationTrigger {
    /// Validation was explicitly requested for the initial form draft.
    Initial,
    /// Validation was requested after a field value changed.
    Change,
    /// Validation was requested directly by application code.
    Manual,
    /// Validation was requested after a field blur event.
    Blur,
    /// Validation was requested as part of a submit attempt.
    Submit,
}

/// The validation triggers a registered validator should run for.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationTriggers {
    triggers: Option<BTreeSet<ValidationTrigger>>,
}

impl ValidationTriggers {
    /// Creates a trigger set that runs for every validation trigger.
    pub fn all() -> Self {
        Self { triggers: None }
    }

    /// Creates a trigger set that runs only for one validation trigger.
    pub fn one(trigger: ValidationTrigger) -> Self {
        Self {
            triggers: Some(BTreeSet::from([trigger])),
        }
    }

    /// Creates a trigger set that runs for the listed validation triggers.
    ///
    /// Duplicate triggers are ignored. An empty set is valid and never runs.
    pub fn new<Triggers>(triggers: Triggers) -> Self
    where
        Triggers: IntoIterator<Item = ValidationTrigger>,
    {
        Self {
            triggers: Some(triggers.into_iter().collect()),
        }
    }

    /// Returns whether this trigger set includes every validation trigger.
    pub const fn is_all(&self) -> bool {
        self.triggers.is_none()
    }

    /// Returns whether validators with this trigger set should run for `trigger`.
    pub fn contains(&self, trigger: ValidationTrigger) -> bool {
        match &self.triggers {
            Some(triggers) => triggers.contains(&trigger),
            None => true,
        }
    }
}

impl Default for ValidationTriggers {
    fn default() -> Self {
        Self::all()
    }
}

impl From<ValidationTrigger> for ValidationTriggers {
    fn from(value: ValidationTrigger) -> Self {
        Self::one(value)
    }
}

impl<const N: usize> From<[ValidationTrigger; N]> for ValidationTriggers {
    fn from(value: [ValidationTrigger; N]) -> Self {
        Self::new(value)
    }
}

impl FromIterator<ValidationTrigger> for ValidationTriggers {
    fn from_iter<Triggers>(triggers: Triggers) -> Self
    where
        Triggers: IntoIterator<Item = ValidationTrigger>,
    {
        Self::new(triggers)
    }
}

impl Extend<ValidationTrigger> for ValidationTriggers {
    fn extend<Triggers>(&mut self, triggers: Triggers)
    where
        Triggers: IntoIterator<Item = ValidationTrigger>,
    {
        let Some(existing_triggers) = &mut self.triggers else {
            return;
        };

        for trigger in triggers {
            existing_triggers.insert(trigger);
        }
    }
}

/// Public policy for automatic validation runs caused by semantic form events.
///
/// The policy controls when validation executes. It does not control error visibility;
/// the default visibility policy still waits for blur or submit attempts.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationMode {
    #[cfg_attr(feature = "serde", serde(default = "default_validate_on_blur"))]
    validate_on_blur: bool,
    validate_on_change: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    revalidate_on_blur_after_submit: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    revalidate_on_change_after_submit: bool,
}

#[cfg(feature = "serde")]
fn default_validate_on_blur() -> bool {
    true
}

impl ValidationMode {
    /// Creates a submit-only mode that does not validate on blur or change.
    pub const fn on_submit() -> Self {
        Self {
            validate_on_blur: false,
            validate_on_change: false,
            revalidate_on_blur_after_submit: false,
            revalidate_on_change_after_submit: false,
        }
    }

    /// Creates the default mode: validate on blur and submit, but not on every change.
    pub const fn on_blur() -> Self {
        Self {
            validate_on_blur: true,
            validate_on_change: false,
            revalidate_on_blur_after_submit: false,
            revalidate_on_change_after_submit: false,
        }
    }

    /// Creates the default mode: validate on blur and submit, but not on every change.
    pub const fn on_blur_or_submit() -> Self {
        Self::on_blur()
    }

    /// Creates a mode that validates after field value changes as well as blur and submit.
    pub const fn on_change() -> Self {
        Self {
            validate_on_blur: true,
            validate_on_change: true,
            revalidate_on_blur_after_submit: false,
            revalidate_on_change_after_submit: false,
        }
    }

    /// Creates a mode that validates on submit first, then revalidates on blur and change after a submit attempt.
    ///
    /// This preserves submit-triggered validation correctness while avoiding live validation before
    /// the user has tried to submit the form.
    pub const fn submit_then_revalidate() -> Self {
        Self {
            validate_on_blur: false,
            validate_on_change: false,
            revalidate_on_blur_after_submit: true,
            revalidate_on_change_after_submit: true,
        }
    }

    /// Enables immediate blur validation on this mode.
    pub const fn validate_on_blur(mut self) -> Self {
        self.validate_on_blur = true;
        self
    }

    /// Configures whether immediate blur validation runs after field blur events.
    pub const fn with_blur_validation(mut self, enabled: bool) -> Self {
        self.validate_on_blur = enabled;
        self
    }

    /// Returns whether this mode validates immediately after field blur events.
    ///
    /// Use [`ValidationMode::should_validate_on_blur`] when submit-attempt-aware behavior matters.
    pub const fn validates_on_blur(self) -> bool {
        self.validate_on_blur
    }

    /// Returns whether this mode should validate after a blur event for the current submit history.
    pub const fn should_validate_on_blur(self, submit_attempts: u64) -> bool {
        self.validate_on_blur || (self.revalidate_on_blur_after_submit && submit_attempts > 0)
    }

    /// Enables immediate change validation on this mode.
    pub const fn validate_on_change(mut self) -> Self {
        self.validate_on_change = true;
        self
    }

    /// Configures whether immediate change validation runs after field value updates.
    pub const fn with_change_validation(mut self, enabled: bool) -> Self {
        self.validate_on_change = enabled;
        self
    }

    /// Returns whether this mode validates immediately after field value updates.
    ///
    /// Use [`ValidationMode::should_validate_on_change`] when submit-attempt-aware behavior matters.
    pub const fn validates_on_change(self) -> bool {
        self.validate_on_change
    }

    /// Returns whether this mode should validate after a value change for the current submit history.
    pub const fn should_validate_on_change(self, submit_attempts: u64) -> bool {
        self.validate_on_change || (self.revalidate_on_change_after_submit && submit_attempts > 0)
    }
}

impl Default for ValidationMode {
    fn default() -> Self {
        Self::on_blur()
    }
}

/// Policy controlling when stored validation errors are exposed through visible-error selectors.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorVisibilityPolicy {
    /// Show field errors after blur and all errors after a submit attempt.
    #[default]
    BlurOrSubmit,
    /// Show field errors after the field is touched and all errors after a submit attempt.
    TouchedOrSubmit,
    /// Show errors only after a submit attempt.
    SubmitOnly,
    /// Show stored errors immediately.
    Always,
}

/// Source-level validation status.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ValidationStatus {
    /// The validator has not produced a result for the current validation lifecycle.
    #[default]
    Unknown,
    /// The validator ran and returned no errors.
    Valid,
    /// The validator ran and returned at least one error.
    Invalid,
    /// The validator is waiting for an asynchronous result.
    Pending,
    /// The validator was intentionally not run for this validation chain.
    Skipped,
    /// The validator result is known to be for an older value or run.
    Stale,
}

mod collection_addressing;
mod field_store;
mod submission;
mod validation_chain;
mod validation_lifecycle;

use field_store::FieldStore;
use submission::SubmissionState;

#[doc(hidden)]
pub mod __private {
    pub use super::collection_addressing::CollectionItemFieldAddress;
}

use collection_addressing::CollectionItemFieldAddress;
use validation_chain::{
    CollectionItemValidatorTemplateKey, RegisteredCollectionItemFieldValidator,
    RegisteredFieldValidator, RegisteredFormValidator, ValidationChainRegistry, ValidatorKey,
    collection_item_template_key_for_field,
};

/// A known reason submission is currently unavailable.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SubmitBlocker {
    /// Stored validation errors currently block submission.
    ValidationErrors,
    /// Mounted input bindings currently have parse errors.
    ParseErrors,
    /// Required validation is currently pending.
    PendingValidation,
    /// A submission has started and has not completed yet.
    InFlightSubmission,
}

/// UI-oriented submit availability based on current known blockers.
///
/// This is a conservative convenience signal for rendering controls and explanations. It can be
/// stricter than submit authority because stored non-submit validation errors are still known UI
/// blockers, even though submit itself reruns submit-triggered validation before calling handlers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SubmitAvailability {
    blockers: Vec<SubmitBlocker>,
}

impl SubmitAvailability {
    /// Creates an available submit state.
    pub fn available() -> Self {
        Self {
            blockers: Vec::new(),
        }
    }

    pub fn blocked_by<Blockers>(blockers: Blockers) -> Self
    where
        Blockers: IntoIterator<Item = SubmitBlocker>,
    {
        Self {
            blockers: blockers.into_iter().collect(),
        }
    }

    /// Returns whether there are no current known submit blockers.
    pub fn is_available(&self) -> bool {
        self.blockers.is_empty()
    }

    /// Returns the known blockers in deterministic order.
    pub fn blockers(&self) -> &[SubmitBlocker] {
        &self.blockers
    }

    /// Returns whether this availability includes a blocker.
    pub fn contains(&self, blocker: SubmitBlocker) -> bool {
        self.blockers.contains(&blocker)
    }
}

/// An owned form value captured at a point in time for async work or application reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormSnapshot<Model> {
    value: Model,
}

impl<Model> FormSnapshot<Model> {
    /// Creates a form snapshot from an owned form model value.
    pub fn new(value: Model) -> Self {
        Self { value }
    }

    /// Returns the captured form model value.
    pub fn value(&self) -> &Model {
        &self.value
    }

    /// Consumes the snapshot and returns the captured form model value.
    pub fn into_value(self) -> Model {
        self.value
    }
}

impl<Model> AsRef<Model> for FormSnapshot<Model> {
    fn as_ref(&self) -> &Model {
        self.value()
    }
}

#[derive(Clone)]
pub(crate) struct SubmitIntentSnapshot {
    value: Rc<dyn Any>,
}

impl fmt::Debug for SubmitIntentSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SubmitIntentSnapshot(..)")
    }
}

impl PartialEq for SubmitIntentSnapshot {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.value, &other.value)
    }
}

impl Eq for SubmitIntentSnapshot {}

impl SubmitIntentSnapshot {
    fn new<Intent>(intent: Intent) -> Self
    where
        Intent: 'static,
    {
        Self {
            value: Rc::new(intent),
        }
    }

    fn get<Intent>(&self) -> Option<&Intent>
    where
        Intent: 'static,
    {
        self.value.downcast_ref()
    }

    fn cloned<Intent>(&self) -> Option<Intent>
    where
        Intent: Clone + 'static,
    {
        self.get().cloned()
    }

    fn matches<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.get::<Intent>() == Some(intent)
    }

    fn as_any(&self) -> &dyn Any {
        self.value.as_ref()
    }
}

/// Owned information supplied to asynchronous validators.
#[derive(Clone)]
pub struct AsyncValidatorContext<Model> {
    form: FormSnapshot<Model>,
    source: ValidatorSource,
    trigger: ValidationTrigger,
    submit_intent: Option<SubmitIntentSnapshot>,
}

impl<Model> AsyncValidatorContext<Model> {
    fn new(
        form: FormSnapshot<Model>,
        source: ValidatorSource,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
    ) -> Self {
        Self {
            form,
            source,
            trigger,
            submit_intent,
        }
    }

    /// Returns the owned form snapshot captured for this validation run.
    pub const fn form_snapshot(&self) -> &FormSnapshot<Model> {
        &self.form
    }

    /// Consumes this context and returns the owned form snapshot.
    pub fn into_form_snapshot(self) -> FormSnapshot<Model> {
        self.form
    }

    /// Returns the captured form model value.
    pub fn value(&self) -> &Model {
        self.form.value()
    }

    /// Returns the validator source for this validation run.
    pub const fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the trigger for this validation run.
    pub const fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    /// Returns the submit intent when this is submit-triggered validation with a matching intent type.
    pub fn submit_intent<Intent>(&self) -> Option<&Intent>
    where
        Intent: 'static,
    {
        self.submit_intent.as_ref()?.get()
    }
}

impl<Model> AsRef<Model> for AsyncValidatorContext<Model> {
    fn as_ref(&self) -> &Model {
        self.value()
    }
}

/// An owned submitted value snapshot captured after submit validation passes.
#[derive(Clone)]
pub struct SubmissionSnapshot<Model, Intent = ()> {
    value: Model,
    intent: Intent,
    field_versions: BTreeMap<FieldIdentity, u64>,
}

/// A point-in-time freshness token for submit validation managed outside the core.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitValidationSnapshot<Intent = ()> {
    form_version: u64,
    field_versions: BTreeMap<FieldIdentity, u64>,
    intent: Intent,
}

/// An owned field-validation snapshot captured before async work leaves the form core.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsyncFieldValidation<Model, Value> {
    form: FormSnapshot<Model>,
    field_value: Value,
    field: FieldIdentity,
    validator_id: ValidatorId,
    source: ValidatorSource,
    trigger: ValidationTrigger,
    submit_intent: Option<SubmitIntentSnapshot>,
    form_version: u64,
    field_version: u64,
    run_id: u64,
}

/// A delayed asynchronous field-validation run waiting for its debounce delay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebouncedAsyncFieldValidation {
    field: FieldIdentity,
    validator_id: ValidatorId,
    source: ValidatorSource,
    trigger: ValidationTrigger,
    run_id: u64,
}

/// A delayed asynchronous form-validation run waiting for its debounce delay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebouncedAsyncFormValidation {
    validator_id: ValidatorId,
    source: ValidatorSource,
    trigger: ValidationTrigger,
    run_id: u64,
}

/// An owned form-validation snapshot captured before async work leaves the form core.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsyncFormValidation<Model> {
    form: FormSnapshot<Model>,
    validator_id: ValidatorId,
    source: ValidatorSource,
    trigger: ValidationTrigger,
    submit_intent: Option<SubmitIntentSnapshot>,
    form_version: u64,
    run_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AsyncValidationRun {
    target: ValidationTarget,
    trigger: ValidationTrigger,
    form_version: u64,
    field_version: Option<u64>,
    run_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DebouncedAsyncValidationRun {
    target: ValidationTarget,
    validator_id: ValidatorId,
    trigger: ValidationTrigger,
    run_id: u64,
}

impl DebouncedAsyncValidationRun {
    fn matches_validator(&self, target: &ValidationTarget, id: ValidatorId) -> bool {
        self.target == *target && self.validator_id == id
    }
}

impl<Model, Value> AsyncFieldValidation<Model, Value> {
    /// Returns the owned form snapshot captured for this validation run.
    pub fn form_snapshot(&self) -> &FormSnapshot<Model> {
        &self.form
    }

    /// Returns the owned field value captured for this validation run.
    pub fn field_value(&self) -> &Value {
        &self.field_value
    }

    /// Returns owned validator context for this async validation run.
    pub fn validator_context(&self) -> AsyncValidatorContext<Model>
    where
        Model: Clone,
    {
        AsyncValidatorContext::new(
            self.form.clone(),
            self.source.clone(),
            self.trigger,
            self.submit_intent.clone(),
        )
    }

    /// Returns the field being validated.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field.clone()
    }

    /// Returns the stable identity of the validator being run.
    pub const fn validator_id(&self) -> ValidatorId {
        self.validator_id
    }

    /// Returns the source label of the validator being run.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the submit intent for submit-triggered validation when the requested type matches.
    pub fn submit_intent<Intent>(&self) -> Option<&Intent>
    where
        Intent: 'static,
    {
        self.submit_intent.as_ref()?.get()
    }

    /// Returns the trigger for this validation run.
    pub const fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    fn lifecycle_run(&self) -> AsyncValidationRun {
        AsyncValidationRun {
            target: ValidationTarget::Field(self.field.clone()),
            trigger: self.trigger,
            form_version: self.form_version,
            field_version: Some(self.field_version),
            run_id: self.run_id,
        }
    }
}

impl DebouncedAsyncFieldValidation {
    /// Returns the field being validated.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field.clone()
    }

    /// Returns the stable identity of the validator being delayed.
    pub const fn validator_id(&self) -> ValidatorId {
        self.validator_id
    }

    /// Returns the source label of the validator being delayed.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the trigger for this delayed run.
    pub const fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    fn lifecycle_run(&self) -> DebouncedAsyncValidationRun {
        DebouncedAsyncValidationRun {
            target: ValidationTarget::Field(self.field.clone()),
            validator_id: self.validator_id,
            trigger: self.trigger,
            run_id: self.run_id,
        }
    }
}

impl DebouncedAsyncFormValidation {
    /// Returns the stable identity of the validator being delayed.
    pub const fn validator_id(&self) -> ValidatorId {
        self.validator_id
    }

    /// Returns the source label of the validator being delayed.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the trigger for this delayed run.
    pub const fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    fn lifecycle_run(&self) -> DebouncedAsyncValidationRun {
        DebouncedAsyncValidationRun {
            target: ValidationTarget::Form,
            validator_id: self.validator_id,
            trigger: self.trigger,
            run_id: self.run_id,
        }
    }
}

impl<Model> AsyncFormValidation<Model> {
    /// Returns the owned form snapshot captured for this validation run.
    pub fn form_snapshot(&self) -> &FormSnapshot<Model> {
        &self.form
    }

    /// Returns owned validator context for this async validation run.
    pub fn validator_context(&self) -> AsyncValidatorContext<Model>
    where
        Model: Clone,
    {
        AsyncValidatorContext::new(
            self.form.clone(),
            self.source.clone(),
            self.trigger,
            self.submit_intent.clone(),
        )
    }

    /// Returns the stable identity of the validator being run.
    pub const fn validator_id(&self) -> ValidatorId {
        self.validator_id
    }

    /// Returns the source label of the validator being run.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the submit intent for submit-triggered validation when the requested type matches.
    pub fn submit_intent<Intent>(&self) -> Option<&Intent>
    where
        Intent: 'static,
    {
        self.submit_intent.as_ref()?.get()
    }

    /// Returns the trigger for this validation run.
    pub const fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    fn lifecycle_run(&self) -> AsyncValidationRun {
        AsyncValidationRun {
            target: ValidationTarget::Form,
            trigger: self.trigger,
            form_version: self.form_version,
            field_version: None,
            run_id: self.run_id,
        }
    }
}

impl<Model: fmt::Debug, Intent: fmt::Debug> fmt::Debug for SubmissionSnapshot<Model, Intent> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SubmissionSnapshot")
            .field("value", &self.value)
            .field("intent", &self.intent)
            .finish()
    }
}

impl<Model: PartialEq, Intent: PartialEq> PartialEq for SubmissionSnapshot<Model, Intent> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.intent == other.intent
    }
}

impl<Model: Eq, Intent: Eq> Eq for SubmissionSnapshot<Model, Intent> {}

impl<Model> SubmissionSnapshot<Model> {
    /// Creates a submission snapshot from an owned form model snapshot without a distinct intent.
    pub fn new(value: Model) -> Self {
        Self::with_intent(value, ())
    }
}

impl<Model, Intent> SubmissionSnapshot<Model, Intent> {
    /// Creates a submission snapshot from an owned form model snapshot and submit intent.
    pub fn with_intent(value: Model, intent: Intent) -> Self {
        Self {
            value,
            intent,
            field_versions: BTreeMap::new(),
        }
    }

    fn with_intent_and_field_versions(
        value: Model,
        intent: Intent,
        field_versions: BTreeMap<FieldIdentity, u64>,
    ) -> Self {
        Self {
            value,
            intent,
            field_versions,
        }
    }

    /// Returns the submitted value snapshot.
    pub fn value(&self) -> &Model {
        &self.value
    }

    /// Returns the submit intent captured with this submission snapshot.
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Consumes the submitted value and returns the owned form model snapshot.
    pub fn into_value(self) -> Model {
        self.value
    }

    /// Consumes the snapshot and returns the owned form model value and submit intent.
    pub fn into_parts(self) -> (Model, Intent) {
        (self.value, self.intent)
    }

    fn field_version(&self, field: &FieldIdentity) -> u64 {
        self.field_versions.get(field).copied().unwrap_or_default()
    }
}

impl<Intent> SubmitValidationSnapshot<Intent> {
    fn new(
        form_version: u64,
        field_versions: BTreeMap<FieldIdentity, u64>,
        intent: Intent,
    ) -> Self {
        Self {
            form_version,
            field_versions,
            intent,
        }
    }

    /// Returns the submit intent captured for this validation snapshot.
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }
}

impl<Model, Intent> AsRef<Model> for SubmissionSnapshot<Model, Intent> {
    fn as_ref(&self) -> &Model {
        self.value()
    }
}

/// The result of starting a submission lifecycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmitAttempt<Model, Intent = ()> {
    /// Submission started with an owned submitted value snapshot.
    Started(SubmissionSnapshot<Model, Intent>),
    /// Submission did not start because of a known blocker.
    Blocked(SubmitBlocker),
}

impl<Model, Intent> SubmitAttempt<Model, Intent> {
    /// Returns whether submission started.
    pub const fn is_started(&self) -> bool {
        matches!(self, Self::Started(_))
    }

    /// Returns whether submission was blocked.
    pub const fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }

    /// Returns the blocker when submission did not start.
    pub const fn blocker(&self) -> Option<SubmitBlocker> {
        match self {
            Self::Started(_) => None,
            Self::Blocked(blocker) => Some(*blocker),
        }
    }

    /// Returns the blocker, panicking if submission started.
    ///
    /// This is the shared guard for submit paths that block *before* submission can start (a
    /// parse-error preflight or a duplicate-submission check), where `Started` is structurally
    /// impossible. It concentrates that invariant so callers do not each re-assert it.
    pub fn expect_blocker(&self) -> SubmitBlocker {
        self.blocker()
            .expect("a pre-submission guard must not produce a started submission")
    }

    /// Consumes this attempt and returns the submission snapshot when submission started.
    pub fn into_started(self) -> Option<SubmissionSnapshot<Model, Intent>> {
        match self {
            Self::Started(submitted) => Some(submitted),
            Self::Blocked(_) => None,
        }
    }
}

/// The result of a submit attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitResult {
    /// Async submission was accepted and will complete later.
    Started,
    /// Submit validation passed and application submit behavior completed successfully.
    Succeeded,
    /// Application submit behavior returned structured submit errors.
    Rejected,
    /// Submission did not start because of a known blocker.
    Blocked(SubmitBlocker),
}

impl SubmitResult {
    /// Returns whether an async submission was accepted and will complete later.
    pub const fn is_started(self) -> bool {
        matches!(self, Self::Started)
    }

    /// Returns whether submit behavior completed successfully.
    pub const fn is_succeeded(self) -> bool {
        matches!(self, Self::Succeeded)
    }

    /// Returns whether submit behavior returned structured submit errors.
    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    /// Returns whether submission did not start because of a known blocker.
    pub const fn is_blocked(self) -> bool {
        matches!(self, Self::Blocked(_))
    }

    /// Returns the blocker when submission did not start.
    pub const fn blocker(self) -> Option<SubmitBlocker> {
        match self {
            Self::Started | Self::Succeeded | Self::Rejected => None,
            Self::Blocked(blocker) => Some(blocker),
        }
    }
}

/// The latest meaningful submission outcome recorded by the form.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitStatus {
    /// Submit validation passed and application submit behavior completed successfully.
    Succeeded,
    /// Application submit behavior returned structured submit errors.
    Rejected,
    /// Submission did not start because of a known blocker.
    Blocked(SubmitBlocker),
}

impl SubmitStatus {
    /// Returns whether the latest submit behavior completed successfully.
    pub const fn is_succeeded(self) -> bool {
        matches!(self, Self::Succeeded)
    }

    /// Returns whether the latest submit behavior returned structured submit errors.
    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    /// Returns whether the latest submission did not start because of a known blocker.
    pub const fn is_blocked(self) -> bool {
        matches!(self, Self::Blocked(_))
    }

    /// Returns the blocker when submission did not start.
    pub const fn blocker(self) -> Option<SubmitBlocker> {
        match self {
            Self::Succeeded | Self::Rejected => None,
            Self::Blocked(blocker) => Some(blocker),
        }
    }
}

/// The latest meaningful submission outcome together with the intent that caused it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LastSubmitStatus<Intent = ()> {
    status: SubmitStatus,
    intent: Intent,
}

impl<Intent> LastSubmitStatus<Intent> {
    /// Creates an intent-associated submit status.
    pub const fn new(status: SubmitStatus, intent: Intent) -> Self {
        Self { status, intent }
    }

    /// Returns the latest submission outcome.
    pub const fn status(&self) -> SubmitStatus {
        self.status
    }

    /// Returns the submit intent associated with the outcome.
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Consumes the status and returns its parts.
    pub fn into_parts(self) -> (SubmitStatus, Intent) {
        (self.status, self.intent)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StoredLastSubmitStatus {
    status: SubmitStatus,
    intent: SubmitIntentSnapshot,
}

impl StoredLastSubmitStatus {
    fn new<Intent>(status: SubmitStatus, intent: Intent) -> Self
    where
        Intent: 'static,
    {
        Self {
            status,
            intent: SubmitIntentSnapshot::new(intent),
        }
    }

    fn with_snapshot(status: SubmitStatus, intent: SubmitIntentSnapshot) -> Self {
        Self { status, intent }
    }

    fn typed<Intent>(&self) -> Option<LastSubmitStatus<Intent>>
    where
        Intent: Clone + 'static,
    {
        Some(LastSubmitStatus::new(
            self.status,
            self.intent.cloned::<Intent>()?,
        ))
    }

    fn matches_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.intent.matches(intent)
    }
}

/// The attachment point for a validation result.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValidationTarget {
    /// A validation result that applies to the form as a whole.
    Form,
    /// A validation result attached to one field.
    Field(FieldIdentity),
}

impl ValidationTarget {
    /// Creates a form-level validation target.
    pub const fn form() -> Self {
        Self::Form
    }

    /// Creates a field-level validation target from a typed field path.
    pub fn field<Model, Value>(path: FieldPath<Model, Value>) -> Self {
        Self::Field(path.identity())
    }

    /// Creates a field-level validation target from a field identity.
    pub fn field_identity(field: FieldIdentity) -> Self {
        Self::Field(field)
    }

    /// Returns whether this target is the form as a whole.
    pub fn is_form(&self) -> bool {
        matches!(self, Self::Form)
    }

    /// Returns the attached field, if this target is field-level.
    pub fn as_field(&self) -> Option<&FieldIdentity> {
        match self {
            Self::Form => None,
            Self::Field(field) => Some(field),
        }
    }
}

type SubmitErrorCurrentCheck<Model> = dyn Fn(&Model, &Model) -> bool + 'static;

/// One structured error returned by application submit behavior.
pub struct SubmitError<Model, Error> {
    target: ValidationTarget,
    error: Error,
    applies_to_current: Option<Box<SubmitErrorCurrentCheck<Model>>>,
}

impl<Model, Error: fmt::Debug> fmt::Debug for SubmitError<Model, Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SubmitError")
            .field("target", &self.target)
            .field("error", &self.error)
            .finish()
    }
}

impl<Model, Error> SubmitError<Model, Error> {
    /// Creates a submit error that applies to the form as a whole.
    pub fn form(error: Error) -> Self {
        Self {
            target: ValidationTarget::Form,
            error,
            applies_to_current: None,
        }
    }

    /// Creates a submit error attached to a typed field path with value-based stale checks.
    ///
    /// The field value is compared against the submitted snapshot before this error is stored.
    /// If the current field value has changed, the returned submit error is discarded as stale.
    pub fn field<Value>(path: FieldPath<Model, Value>, error: Error) -> Self
    where
        Model: 'static,
        Value: PartialEq + 'static,
    {
        let field = path.clone();

        Self {
            target: ValidationTarget::Field(path.identity()),
            error,
            applies_to_current: Some(Box::new(move |current, submitted| {
                field.get(current) == field.get(submitted)
            })),
        }
    }

    /// Creates a submit error attached to a field identity without requiring field equality.
    ///
    /// This is intended for field values that cannot be compared. When this error is stored
    /// through the normal submission APIs, the core keeps it only if the field has not changed
    /// since the submitted snapshot.
    pub fn field_identity(field: FieldIdentity, error: Error) -> Self {
        Self {
            target: ValidationTarget::Field(field),
            error,
            applies_to_current: None,
        }
    }

    /// Returns where this submit error is attached.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }

    /// Returns the typed validation error.
    pub const fn error(&self) -> &Error {
        &self.error
    }

    /// Consumes this submit error and returns the typed validation error.
    pub fn into_error(self) -> Error {
        self.error
    }

    fn applies_to(&self, current: &Model, submitted: &Model) -> bool {
        self.applies_to_current
            .as_ref()
            .map(|applies| applies(current, submitted))
            .unwrap_or(true)
    }
}

/// Structured submit errors returned by application submit behavior.
pub struct SubmitErrors<Model, Error> {
    source: ValidatorSource,
    errors: Vec<SubmitError<Model, Error>>,
}

impl<Model, Error: fmt::Debug> fmt::Debug for SubmitErrors<Model, Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SubmitErrors")
            .field("source", &self.source)
            .field("errors", &self.errors)
            .finish()
    }
}

impl<Model, Error> SubmitErrors<Model, Error> {
    /// Creates an empty successful submit result.
    pub fn none() -> Self {
        Self {
            source: ValidatorSource::submit(),
            errors: Vec::new(),
        }
    }

    /// Creates submit errors using the default submit validation source.
    pub fn new<Errors>(errors: Errors) -> Self
    where
        Errors: IntoIterator<Item = SubmitError<Model, Error>>,
    {
        Self::with_source(ValidatorSource::submit(), errors)
    }

    /// Creates submit errors using a custom submit-related validation source.
    pub fn with_source<Source, Errors>(source: Source, errors: Errors) -> Self
    where
        Source: Into<ValidatorSource>,
        Errors: IntoIterator<Item = SubmitError<Model, Error>>,
    {
        Self {
            source: source.into(),
            errors: errors.into_iter().collect(),
        }
    }

    /// Returns the validation source used when storing these submit errors.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the structured submit errors.
    pub fn errors(&self) -> &[SubmitError<Model, Error>] {
        &self.errors
    }

    /// Returns whether no submit errors were returned.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Adds one submit error.
    pub fn push(&mut self, error: SubmitError<Model, Error>) {
        self.errors.push(error);
    }

    fn into_parts(self) -> (ValidatorSource, Vec<SubmitError<Model, Error>>) {
        (self.source, self.errors)
    }
}

impl<Model, Error> Default for SubmitErrors<Model, Error> {
    fn default() -> Self {
        Self::none()
    }
}

impl<Model, Error> Extend<SubmitError<Model, Error>> for SubmitErrors<Model, Error> {
    fn extend<Errors>(&mut self, errors: Errors)
    where
        Errors: IntoIterator<Item = SubmitError<Model, Error>>,
    {
        self.errors.extend(errors);
    }
}

impl<Model, Error> FromIterator<SubmitError<Model, Error>> for SubmitErrors<Model, Error> {
    fn from_iter<Errors>(errors: Errors) -> Self
    where
        Errors: IntoIterator<Item = SubmitError<Model, Error>>,
    {
        Self::new(errors)
    }
}

impl<Model, Error> IntoIterator for SubmitErrors<Model, Error> {
    type Item = SubmitError<Model, Error>;
    type IntoIter = std::vec::IntoIter<SubmitError<Model, Error>>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.into_iter()
    }
}

impl<'a, Model, Error> IntoIterator for &'a SubmitErrors<Model, Error> {
    type Item = &'a SubmitError<Model, Error>;
    type IntoIter = std::slice::Iter<'a, SubmitError<Model, Error>>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.iter()
    }
}

impl<Model, Error> From<()> for SubmitErrors<Model, Error> {
    fn from(_value: ()) -> Self {
        Self::none()
    }
}

impl<Model, Error> From<SubmitError<Model, Error>> for SubmitErrors<Model, Error> {
    fn from(value: SubmitError<Model, Error>) -> Self {
        Self::new([value])
    }
}

impl<Model, Error> From<Vec<SubmitError<Model, Error>>> for SubmitErrors<Model, Error> {
    fn from(value: Vec<SubmitError<Model, Error>>) -> Self {
        Self::new(value)
    }
}

impl<Model, Error, Rejection> From<Result<(), Rejection>> for SubmitErrors<Model, Error>
where
    Rejection: Into<SubmitErrors<Model, Error>>,
{
    fn from(value: Result<(), Rejection>) -> Self {
        match value {
            Ok(()) => Self::none(),
            Err(errors) => errors.into(),
        }
    }
}

/// Read-only information supplied to validators.
pub struct ValidatorContext<'a, Model> {
    form: &'a Model,
    field_identity: FieldIdentity,
    validator_id: ValidatorId,
    source: ValidatorSource,
    trigger: ValidationTrigger,
    field_metadata: FieldMetadata,
    submit_intent: Option<&'a dyn Any>,
}

impl<'a, Model> ValidatorContext<'a, Model> {
    /// Returns the current form draft snapshot being validated.
    pub fn form(&self) -> &'a Model {
        self.form
    }

    /// Returns the field currently being validated.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field_identity.clone()
    }

    /// Returns the stable identity of the validator currently running.
    pub const fn validator_id(&self) -> ValidatorId {
        self.validator_id
    }

    /// Returns the source label of the validator currently running.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the trigger for this validation run.
    pub const fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    /// Returns read-only interaction metadata for the field being validated.
    pub const fn field_metadata(&self) -> FieldMetadata {
        self.field_metadata
    }

    /// Returns the submit intent for submit-triggered validation when the requested type matches.
    pub fn submit_intent<Intent>(&self) -> Option<&'a Intent>
    where
        Intent: 'static,
    {
        self.submit_intent?.downcast_ref()
    }
}

/// Read-only information supplied to form validators.
pub struct FormValidatorContext<'a, Model> {
    form: &'a Model,
    validator_id: ValidatorId,
    source: ValidatorSource,
    trigger: ValidationTrigger,
    field_store: &'a FieldStore,
    submit_intent: Option<&'a dyn Any>,
}

impl<'a, Model> FormValidatorContext<'a, Model> {
    /// Returns the current form draft snapshot being validated.
    pub fn form(&self) -> &'a Model {
        self.form
    }

    /// Returns the stable identity of the validator currently running.
    pub const fn validator_id(&self) -> ValidatorId {
        self.validator_id
    }

    /// Returns the source label of the validator currently running.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the trigger for this validation run.
    pub const fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    /// Returns read-only interaction metadata for a typed field path.
    pub fn field_metadata<Value>(&self, path: FieldPath<Model, Value>) -> FieldMetadata {
        self.field_metadata_by_identity(&path.identity())
    }

    /// Returns read-only interaction metadata for a field identity.
    pub fn field_metadata_by_identity(&self, field: &FieldIdentity) -> FieldMetadata {
        self.field_store.metadata(field)
    }

    /// Returns the submit intent for submit-triggered validation when the requested type matches.
    pub fn submit_intent<Intent>(&self) -> Option<&'a Intent>
    where
        Intent: 'static,
    {
        self.submit_intent?.downcast_ref()
    }
}

/// One validation error produced by a form-level validator.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormValidationError<Error> {
    target: ValidationTarget,
    error: Error,
}

impl<Error> FormValidationError<Error> {
    /// Creates a validation error that applies to the form as a whole.
    pub fn form(error: Error) -> Self {
        Self {
            target: ValidationTarget::Form,
            error,
        }
    }

    /// Creates a validation error attached to a typed field path.
    pub fn field<Model, Value>(path: FieldPath<Model, Value>, error: Error) -> Self {
        Self::field_identity(path.identity(), error)
    }

    /// Creates a validation error attached to a field identity.
    pub fn field_identity(field: FieldIdentity, error: Error) -> Self {
        Self {
            target: ValidationTarget::Field(field),
            error,
        }
    }

    /// Creates a validation error attached to an already-resolved target.
    ///
    /// This is the routing primitive for **Validation Adapters**: once an adapter has resolved an
    /// **External Diagnostic Path** to a [`ValidationTarget`], it attaches the mapped error without
    /// re-deciding form-versus-field.
    pub fn for_target(target: ValidationTarget, error: Error) -> Self {
        Self { target, error }
    }

    /// Returns where this validation error is attached.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }

    /// Returns the typed validation error.
    pub const fn error(&self) -> &Error {
        &self.error
    }

    /// Consumes this result and returns the typed validation error.
    pub fn into_error(self) -> Error {
        self.error
    }
}

/// One flattened validation error with its source metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationErrorView<'a, Error> {
    target: ValidationTarget,
    source: &'a ValidatorSource,
    validator_id: Option<ValidatorId>,
    error: &'a Error,
}

impl<'a, Error> ValidationErrorView<'a, Error> {
    /// Returns where this error is attached.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }

    /// Returns the field this error is attached to, if any.
    pub fn field_identity(&self) -> Option<FieldIdentity> {
        self.target.as_field().cloned()
    }

    /// Returns the field this error is attached to, if any.
    pub fn field(&self) -> Option<FieldIdentity> {
        self.field_identity()
    }

    /// Returns the field this error is attached to.
    ///
    /// Panics when called for a form-level error.
    pub fn expect_field(&self) -> FieldIdentity {
        self.field()
            .expect("validation error is not attached to a field")
    }

    /// Returns the validator source that produced this error.
    pub fn source(&self) -> &'a ValidatorSource {
        self.source
    }

    /// Returns the registered validator ID for validator-sourced errors.
    ///
    /// Submit-sourced errors are not produced by a registered validator and return `None`.
    pub const fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Returns the typed validation error.
    pub const fn error(&self) -> &'a Error {
        self.error
    }

    /// Returns an owned snapshot of this validation error view.
    pub fn to_snapshot(&self) -> ValidationErrorSnapshot<Error>
    where
        Error: Clone,
    {
        ValidationErrorSnapshot {
            target: self.target.clone(),
            source: self.source.clone(),
            validator_id: self.validator_id,
            error: self.error.clone(),
        }
    }
}

/// One owned validation error snapshot with its source metadata.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationErrorSnapshot<Error> {
    target: ValidationTarget,
    source: ValidatorSource,
    validator_id: Option<ValidatorId>,
    error: Error,
}

impl<Error> ValidationErrorSnapshot<Error> {
    /// Returns where this error is attached.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }

    /// Returns the field this error is attached to, if any.
    pub fn field_identity(&self) -> Option<FieldIdentity> {
        self.target.as_field().cloned()
    }

    /// Returns the field this error is attached to, if any.
    pub fn field(&self) -> Option<FieldIdentity> {
        self.field_identity()
    }

    /// Returns the field this error is attached to.
    ///
    /// Panics when called for a form-level error.
    pub fn expect_field(&self) -> FieldIdentity {
        self.field()
            .expect("validation error is not attached to a field")
    }

    /// Returns the validator source that produced this error.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the registered validator ID for validator-sourced errors.
    ///
    /// Submit-sourced errors are not produced by a registered validator and return `None`.
    pub const fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Returns the typed validation error.
    pub const fn error(&self) -> &Error {
        &self.error
    }

    /// Consumes this snapshot and returns the typed validation error.
    pub fn into_error(self) -> Error {
        self.error
    }
}

impl<'a, Error: Clone> From<ValidationErrorView<'a, Error>> for ValidationErrorSnapshot<Error> {
    fn from(value: ValidationErrorView<'a, Error>) -> Self {
        value.to_snapshot()
    }
}

/// One flattened validation status with its source metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationStatusView {
    target: ValidationTarget,
    validator_id: ValidatorId,
    source: ValidatorSource,
    status: ValidationStatus,
}

impl ValidationStatusView {
    /// Returns where this status is attached.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }

    /// Returns the field this status is attached to, if any.
    pub fn field_identity(&self) -> Option<FieldIdentity> {
        self.target.as_field().cloned()
    }

    /// Returns the field this status is attached to, if any.
    pub fn field(&self) -> Option<FieldIdentity> {
        self.field_identity()
    }

    /// Returns the field this status is attached to.
    ///
    /// Panics when called for a form-level status.
    pub fn expect_field(&self) -> FieldIdentity {
        self.field()
            .expect("validation status is not attached to a field")
    }

    /// Returns the stable validator ID for this status.
    pub const fn validator_id(&self) -> ValidatorId {
        self.validator_id
    }

    /// Returns the validator source label for this status.
    pub fn source(&self) -> &ValidatorSource {
        &self.source
    }

    /// Returns the source-level validation status.
    pub const fn status(&self) -> ValidationStatus {
        self.status
    }
}

/// Field metadata included in observer events without exposing field values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormObserverField {
    identity: FieldIdentity,
    field_name: String,
}

impl FormObserverField {
    /// Creates observer field metadata from a typed field path.
    pub fn from_path<Model, Value>(path: &FieldPath<Model, Value>) -> Self {
        Self {
            identity: path.identity(),
            field_name: path.field_name().to_owned(),
        }
    }

    fn new(identity: FieldIdentity, field_name: impl Into<String>) -> Self {
        Self {
            identity,
            field_name: field_name.into(),
        }
    }

    /// Returns the internal field identity.
    pub fn identity(&self) -> FieldIdentity {
        self.identity.clone()
    }

    /// Returns the rendered field name for HTML interoperability.
    pub fn field_name(&self) -> &str {
        &self.field_name
    }
}

/// The origin of a field value update observed by the form core.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldUpdateOrigin {
    /// The update was initiated by application code.
    Programmatic,
    /// The update was initiated by user interaction through a field binding.
    User,
}

/// A value marker for observer events.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormObserverValue {
    /// The value was intentionally omitted from the event.
    Redacted,
}

impl FormObserverValue {
    /// Returns whether this observer value marker redacts the actual value.
    pub const fn is_redacted(self) -> bool {
        matches!(self, Self::Redacted)
    }
}

/// A value-redacted form transition event emitted to registered observers.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormObserverEvent {
    /// A typed field value was replaced through the controlled update path.
    #[non_exhaustive]
    FieldUpdated {
        /// The field that changed.
        field: FormObserverField,
        /// Whether the change came from application code or user interaction.
        origin: FieldUpdateOrigin,
        /// The updated field value, redacted by default.
        value: FormObserverValue,
    },
    /// A logical item was inserted into a collection field.
    #[non_exhaustive]
    CollectionItemInserted {
        /// The collection field that changed.
        collection: FieldIdentity,
        /// The logical identity assigned to the item.
        item: CollectionItemIdentity,
        /// The index where the item was inserted.
        index: usize,
        /// Whether the change came from application code or user interaction.
        origin: FieldUpdateOrigin,
        /// The inserted item value, redacted by default.
        value: FormObserverValue,
    },
    /// A logical item was removed from a collection field.
    #[non_exhaustive]
    CollectionItemRemoved {
        /// The collection field that changed.
        collection: FieldIdentity,
        /// The logical identity removed from the collection.
        item: CollectionItemIdentity,
        /// The index the item occupied before removal.
        index: usize,
        /// Whether the change came from application code or user interaction.
        origin: FieldUpdateOrigin,
        /// The removed item value, redacted by default.
        value: FormObserverValue,
    },
    /// A logical item was moved within a collection field.
    #[non_exhaustive]
    CollectionItemMoved {
        /// The collection field that changed.
        collection: FieldIdentity,
        /// The logical identity moved within the collection.
        item: CollectionItemIdentity,
        /// The item's previous index.
        from: usize,
        /// The item's new index.
        to: usize,
        /// Whether the change came from application code or user interaction.
        origin: FieldUpdateOrigin,
    },
    /// Two logical items exchanged positions within a collection field.
    #[non_exhaustive]
    CollectionItemsSwapped {
        /// The collection field that changed.
        collection: FieldIdentity,
        /// The logical identity that was at the first position.
        first: CollectionItemIdentity,
        /// The logical identity that was at the second position.
        second: CollectionItemIdentity,
        /// Whether the change came from application code or user interaction.
        origin: FieldUpdateOrigin,
    },
    /// One logical item's value was replaced in place, keeping its identity.
    #[non_exhaustive]
    CollectionItemReplaced {
        /// The collection field that changed.
        collection: FieldIdentity,
        /// The logical identity whose value was replaced.
        item: CollectionItemIdentity,
        /// The index of the replaced item.
        index: usize,
        /// Whether the change came from application code or user interaction.
        origin: FieldUpdateOrigin,
        /// The new item value, redacted by default.
        value: FormObserverValue,
    },
    /// Every logical item was removed from a collection field.
    #[non_exhaustive]
    CollectionCleared {
        /// The collection field that changed.
        collection: FieldIdentity,
        /// Whether the change came from application code or user interaction.
        origin: FieldUpdateOrigin,
    },
    /// A validator source ran and stored a new source-level status.
    #[non_exhaustive]
    ValidationRan {
        /// The validation attachment point.
        target: ValidationTarget,
        /// The validator source that ran.
        source: ValidatorSource,
        /// The semantic trigger for the validation run.
        trigger: ValidationTrigger,
        /// The resulting source-level status.
        status: ValidationStatus,
    },
    /// An asynchronous validator source was scheduled or marked pending.
    #[non_exhaustive]
    AsyncValidationScheduled {
        /// The validation attachment point.
        target: ValidationTarget,
        /// The validator source that was scheduled.
        source: ValidatorSource,
        /// The semantic trigger for the validation run.
        trigger: ValidationTrigger,
        /// The resulting source-level status.
        status: ValidationStatus,
    },
    /// An asynchronous validator source completed and stored a result.
    #[non_exhaustive]
    AsyncValidationCompleted {
        /// The validation attachment point.
        target: ValidationTarget,
        /// The validator source that completed.
        source: ValidatorSource,
        /// The semantic trigger for the validation run.
        trigger: ValidationTrigger,
        /// The resulting source-level status.
        status: ValidationStatus,
    },
    /// An asynchronous validator source was skipped by validation chain short-circuiting.
    #[non_exhaustive]
    AsyncValidationSkipped {
        /// The validation attachment point.
        target: ValidationTarget,
        /// The validator source that was skipped.
        source: ValidatorSource,
        /// The semantic trigger for the validation run.
        trigger: ValidationTrigger,
        /// The resulting source-level status.
        status: ValidationStatus,
    },
    /// A stale asynchronous validation result was ignored.
    #[non_exhaustive]
    AsyncValidationStaleIgnored {
        /// The validation attachment point.
        target: ValidationTarget,
        /// The validator source whose stale result was ignored.
        source: ValidatorSource,
        /// The semantic trigger for the ignored validation run.
        trigger: ValidationTrigger,
        /// The ignored source-level status.
        status: ValidationStatus,
    },
    /// A debounced asynchronous validator source was scheduled.
    #[non_exhaustive]
    DebouncedAsyncValidationScheduled {
        /// The validation attachment point.
        target: ValidationTarget,
        /// The validator source that was scheduled.
        source: ValidatorSource,
        /// The semantic trigger for the delayed validation run.
        trigger: ValidationTrigger,
        /// The resulting source-level status.
        status: ValidationStatus,
    },
    /// A debounced asynchronous validator source flushed into a runnable async validation.
    #[non_exhaustive]
    DebouncedAsyncValidationFlushed {
        /// The validation attachment point.
        target: ValidationTarget,
        /// The validator source that was flushed.
        source: ValidatorSource,
        /// The semantic trigger for the delayed validation run.
        trigger: ValidationTrigger,
        /// The current source-level status.
        status: ValidationStatus,
    },
    /// A submit attempt was recorded.
    #[non_exhaustive]
    SubmitAttempted {
        /// The current submit attempt count after recording this attempt.
        attempt: u64,
    },
    /// One field was reset to its baseline value.
    #[non_exhaustive]
    FieldReset {
        /// The field that was reset.
        field: FormObserverField,
    },
    /// The form was reset to its baseline value.
    #[non_exhaustive]
    Reset {
        /// The restored form value, redacted by default.
        value: FormObserverValue,
    },
    /// The form was explicitly reinitialized with a new baseline and current value.
    #[non_exhaustive]
    Reinitialized {
        /// The new form value, redacted by default.
        value: FormObserverValue,
    },
}

type FormObserver = dyn FnMut(&FormObserverEvent) + 'static;

/// User interaction metadata tracked for one field.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FieldMetadata {
    touched: bool,
    blurred: bool,
}

impl FieldMetadata {
    /// Returns whether this field has received user interaction.
    pub const fn is_touched(self) -> bool {
        self.touched
    }

    /// Returns whether this field has lost focus at least once.
    pub const fn is_blurred(self) -> bool {
        self.blurred
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CollectionState {
    baseline_items: Vec<CollectionItemIdentity>,
    current_items: Vec<CollectionItemIdentity>,
    next_item_identity: u64,
}

impl CollectionState {
    fn new(baseline_len: usize, current_len: usize) -> Self {
        let shared_len = baseline_len.min(current_len);
        let baseline_items: Vec<_> = (0..baseline_len)
            .map(|index| {
                CollectionItemIdentity(
                    u64::try_from(index).expect("collection item index should fit into u64"),
                )
            })
            .collect();
        let mut current_items: Vec<_> = baseline_items.iter().copied().take(shared_len).collect();
        current_items.extend(
            (baseline_len..baseline_len + current_len.saturating_sub(shared_len)).map(|index| {
                CollectionItemIdentity(
                    u64::try_from(index).expect("collection item index should fit into u64"),
                )
            }),
        );

        Self {
            baseline_items,
            current_items,
            next_item_identity: u64::try_from(baseline_len.max(current_len))
                .expect("collection length should fit into u64"),
        }
    }

    fn items(&self) -> Vec<CollectionItem> {
        self.current_items
            .iter()
            .copied()
            .enumerate()
            .map(|(index, identity)| CollectionItem { identity, index })
            .collect()
    }

    fn current_index(&self, item: CollectionItemIdentity) -> Option<usize> {
        self.current_items
            .iter()
            .position(|candidate| *candidate == item)
    }

    fn baseline_index(&self, item: CollectionItemIdentity) -> Option<usize> {
        self.baseline_items
            .iter()
            .position(|candidate| *candidate == item)
    }

    fn allocate_item_identity(&mut self) -> CollectionItemIdentity {
        let identity = CollectionItemIdentity(self.next_item_identity);
        self.next_item_identity = self
            .next_item_identity
            .checked_add(1)
            .expect("collection item identity counter exhausted");
        identity
    }

    fn is_dirty(&self) -> bool {
        self.current_items != self.baseline_items
    }
}

/// Current compatibility version for serialized form-state snapshots.
pub const FORM_STATE_SERIALIZATION_VERSION: u32 = 3;

/// Current compatibility version for serialized collection identity state.
pub const COLLECTION_IDENTITY_SERIALIZATION_VERSION: u32 = 1;

/// Which collection item identity sequence is malformed in serialized state.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionIdentitySequence {
    /// The baseline identity sequence is malformed.
    Baseline,
    /// The current rendered identity sequence is malformed.
    Current,
}

/// A restore-time compatibility or integrity problem in serialized form state.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FormStateRestoreError {
    /// The serialized form-state format version is not supported by this crate version.
    UnsupportedFormStateVersion { expected: u32, actual: u32 },
    /// The serialized collection identity format version is not supported by this crate version.
    UnsupportedCollectionIdentityVersion { expected: u32, actual: u32 },
    /// One serialized collection identity entry used a collection-item field identity as a collection.
    UnsupportedCollectionIdentity { collection: FieldIdentity },
    /// The serialized collection identity state contains the same collection more than once.
    DuplicateCollectionIdentity { collection: FieldIdentity },
    /// One serialized collection identity sequence contains the same item identity more than once.
    DuplicateCollectionItemIdentity {
        collection: FieldIdentity,
        sequence: CollectionIdentitySequence,
        item: CollectionItemIdentity,
    },
    /// The next identity counter would reuse an identity already present in the collection state.
    InvalidNextCollectionItemIdentity {
        collection: FieldIdentity,
        next_item_identity: u64,
        max_item_identity: u64,
    },
}

/// Serializable identity state for all tracked collection fields in one form snapshot.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionIdentityState {
    version: u32,
    collections: Vec<CollectionIdentitySnapshot>,
}

impl CollectionIdentityState {
    fn from_collection_states(states: &BTreeMap<FieldIdentity, CollectionState>) -> Self {
        Self {
            version: COLLECTION_IDENTITY_SERIALIZATION_VERSION,
            collections: states
                .iter()
                .map(|(collection, state)| CollectionIdentitySnapshot {
                    collection: collection.clone(),
                    baseline_items: state.baseline_items.clone(),
                    current_items: state.current_items.clone(),
                    next_item_identity: state.next_item_identity,
                })
                .collect(),
        }
    }

    fn into_collection_states(
        self,
    ) -> Result<BTreeMap<FieldIdentity, CollectionState>, FormStateRestoreError> {
        if self.version != COLLECTION_IDENTITY_SERIALIZATION_VERSION {
            return Err(
                FormStateRestoreError::UnsupportedCollectionIdentityVersion {
                    expected: COLLECTION_IDENTITY_SERIALIZATION_VERSION,
                    actual: self.version,
                },
            );
        }

        let mut states = BTreeMap::new();

        for collection in self.collections {
            let (identity, state) = collection.into_collection_state()?;

            if states.insert(identity.clone(), state).is_some() {
                return Err(FormStateRestoreError::DuplicateCollectionIdentity {
                    collection: identity,
                });
            }
        }

        Ok(states)
    }

    /// Returns the serialized format version for compatibility checks.
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the serialized per-collection identity states.
    pub fn collections(&self) -> &[CollectionIdentitySnapshot] {
        &self.collections
    }
}

/// Serializable identity sequences for one collection field.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionIdentitySnapshot {
    collection: FieldIdentity,
    baseline_items: Vec<CollectionItemIdentity>,
    current_items: Vec<CollectionItemIdentity>,
    next_item_identity: u64,
}

impl CollectionIdentitySnapshot {
    fn into_collection_state(
        self,
    ) -> Result<(FieldIdentity, CollectionState), FormStateRestoreError> {
        if !self.collection.is_static() {
            return Err(FormStateRestoreError::UnsupportedCollectionIdentity {
                collection: self.collection,
            });
        }

        validate_collection_identity_sequence(
            self.collection.clone(),
            CollectionIdentitySequence::Baseline,
            &self.baseline_items,
        )?;
        validate_collection_identity_sequence(
            self.collection.clone(),
            CollectionIdentitySequence::Current,
            &self.current_items,
        )?;

        let max_item_identity = self
            .baseline_items
            .iter()
            .chain(&self.current_items)
            .map(|identity| identity.as_u64())
            .max();

        if let Some(max_item_identity) = max_item_identity
            && self.next_item_identity <= max_item_identity
        {
            return Err(FormStateRestoreError::InvalidNextCollectionItemIdentity {
                collection: self.collection.clone(),
                next_item_identity: self.next_item_identity,
                max_item_identity,
            });
        }

        Ok((
            self.collection,
            CollectionState {
                baseline_items: self.baseline_items,
                current_items: self.current_items,
                next_item_identity: self.next_item_identity,
            },
        ))
    }

    /// Returns the collection field identity this state belongs to.
    pub fn collection(&self) -> FieldIdentity {
        self.collection.clone()
    }

    /// Returns baseline item identities in baseline order.
    pub fn baseline_items(&self) -> &[CollectionItemIdentity] {
        &self.baseline_items
    }

    /// Returns current item identities in rendered order.
    pub fn current_items(&self) -> &[CollectionItemIdentity] {
        &self.current_items
    }

    /// Returns the next identity counter value for future insertions.
    pub const fn next_item_identity(&self) -> u64 {
        self.next_item_identity
    }
}

fn validate_collection_identity_sequence(
    collection: FieldIdentity,
    sequence: CollectionIdentitySequence,
    items: &[CollectionItemIdentity],
) -> Result<(), FormStateRestoreError> {
    let mut seen = Vec::new();

    for item in items {
        if seen.contains(item) {
            return Err(FormStateRestoreError::DuplicateCollectionItemIdentity {
                collection,
                sequence,
                item: *item,
            });
        }

        seen.push(*item);
    }

    Ok(())
}

/// The editable form draft and its baseline value.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormDraft<Model> {
    baseline: Model,
    current: Model,
}

impl<Model: Clone> FormDraft<Model> {
    /// Creates a draft where the baseline and current value start equal.
    pub fn new(initial: Model) -> Self {
        Self {
            baseline: initial.clone(),
            current: initial,
        }
    }

    /// Restores the current draft value to the baseline value.
    pub fn reset(&mut self) {
        self.current = self.baseline.clone();
    }

    /// Replaces the baseline and current draft value.
    pub fn reinitialize(&mut self, initial: Model) {
        *self = Self::new(initial);
    }
}

impl<Model> FormDraft<Model> {
    /// Returns the baseline value for this form draft.
    pub fn baseline(&self) -> &Model {
        &self.baseline
    }

    /// Returns the current editable value for this form draft.
    pub fn current(&self) -> &Model {
        &self.current
    }

    /// Returns the current editable value for internal state-machine updates.
    fn current_mut(&mut self) -> &mut Model {
        &mut self.current
    }
}

fn collection_item_field_identity<Model, Item, Value>(
    collection: &FieldPath<Model, Vec<Item>>,
    item: CollectionItemIdentity,
    field: &FieldPath<Item, Value>,
) -> FieldIdentity {
    CollectionItemFieldAddress::identity_for(collection, item, field)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoredSubmitError<Error> {
    target: ValidationTarget,
    source: ValidatorSource,
    submit_intent: Option<SubmitIntentSnapshot>,
    error: Error,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct FieldVersionSnapshot {
    field: FieldIdentity,
    version: u64,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct FieldMetadataSnapshot {
    field: FieldIdentity,
    metadata: FieldMetadata,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct FieldValidatorStateSnapshot<Error> {
    field: FieldIdentity,
    validator_id: ValidatorId,
    state: validation_lifecycle::SourceResultSnapshot<Error>,
}

impl<Error> FieldValidatorStateSnapshot<Error> {
    fn key(&self) -> ValidatorKey {
        ValidatorKey::new(self.field.clone(), self.validator_id)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct FormValidatorStateSnapshot<Error> {
    validator_id: ValidatorId,
    state: validation_lifecycle::SourceResultSnapshot<FormValidationError<Error>>,
}

fn form_validation_error_targets_file<Error>(error: &FormValidationError<Error>) -> bool {
    matches!(&error.target, ValidationTarget::Field(field) if field.is_file())
}

fn without_file_targeted_form_validation_errors<Error>(
    mut state: validation_lifecycle::SourceResultSnapshot<FormValidationError<Error>>,
) -> validation_lifecycle::SourceResultSnapshot<FormValidationError<Error>> {
    state.retain_errors(|error| !form_validation_error_targets_file(error));
    state
}

/// Opt-in serialized state for a form core.
///
/// This snapshot intentionally contains no validator closures, observers, Dioxus tasks, or in-flight
/// submissions. Applications restore behavior by constructing their normal form configuration and
/// then applying the snapshot to restore values and stored result state.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormStateSnapshot<Model, Error = String> {
    version: u32,
    draft: FormDraft<Model>,
    validation_mode: ValidationMode,
    error_visibility_policy: ErrorVisibilityPolicy,
    form_version: u64,
    field_versions: Vec<FieldVersionSnapshot>,
    field_metadata: Vec<FieldMetadataSnapshot>,
    collection_identities: CollectionIdentityState,
    field_validator_states: Vec<FieldValidatorStateSnapshot<Error>>,
    collection_item_validator_states: Vec<FieldValidatorStateSnapshot<Error>>,
    form_validator_states: Vec<FormValidatorStateSnapshot<Error>>,
    next_validator_id: u64,
    submit_attempts: u64,
}

impl<Model, Error> FormStateSnapshot<Model, Error> {
    /// Returns the serialized format version for compatibility checks.
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the captured draft state, including baseline and current values.
    pub const fn draft(&self) -> &FormDraft<Model> {
        &self.draft
    }

    /// Returns the collection identity state embedded in this form snapshot.
    pub const fn collection_identity_state(&self) -> &CollectionIdentityState {
        &self.collection_identities
    }

    /// Returns the captured error visibility policy.
    pub const fn error_visibility_policy(&self) -> ErrorVisibilityPolicy {
        self.error_visibility_policy
    }
}

/// Renderer-agnostic form state that owns the form draft.
pub struct FormCore<Model, Error = String> {
    draft: FormDraft<Model>,
    validation_mode: ValidationMode,
    form_version: u64,
    field_store: FieldStore,
    validation_chains: ValidationChainRegistry<Model, Error>,
    submission: SubmissionState<Error>,
    observers: Vec<Box<FormObserver>>,
    error_visibility_policy: ErrorVisibilityPolicy,
}

/// Form-core operations scoped to one submit intent.
pub struct FormCoreIntent<'form, Model, Error, Intent> {
    core: &'form mut FormCore<Model, Error>,
    intent: Intent,
}

impl<Model, Error> FormCore<Model, Error> {
    /// Scopes submit-related operations to one explicit submit intent.
    pub fn intent<Intent>(&mut self, intent: Intent) -> FormCoreIntent<'_, Model, Error, Intent> {
        FormCoreIntent { core: self, intent }
    }
}

impl<Model: Clone> FormCore<Model> {
    /// Creates form state that owns a draft initialized from `initial`.
    pub fn new(initial: Model) -> Self {
        Self::new_with_error_type(initial)
    }

    /// Creates form state with an explicit validation error type.
    pub fn new_with_error_type<Error>(initial: Model) -> FormCore<Model, Error> {
        FormCore {
            draft: FormDraft::new(initial),
            validation_mode: ValidationMode::default(),
            form_version: 0,
            field_store: FieldStore::default(),
            validation_chains: ValidationChainRegistry::new(),
            submission: SubmissionState::default(),
            observers: Vec::new(),
            error_visibility_policy: ErrorVisibilityPolicy::default(),
        }
    }
}

impl<Model: Clone, Error> FormCore<Model, Error> {
    /// Returns an owned snapshot of the current draft value.
    pub fn snapshot(&self) -> Model {
        self.draft.current().clone()
    }

    /// Starts a submission by blocking duplicates, validating, and returning an owned snapshot.
    pub fn begin_submission(&mut self) -> SubmitAttempt<Model> {
        self.intent(()).begin_submission()
    }

    fn begin_intent_submission<Intent>(&mut self, intent: Intent) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + PartialEq + 'static,
    {
        if self.submission.is_in_flight() {
            self.record_submit_status(
                SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission),
                intent,
            );
            return SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission);
        }

        self.clear_submit_errors();
        self.mark_submit_attempt();
        self.submission
            .set_validation_intent(Some(SubmitIntentSnapshot::new(intent.clone())));
        self.validate_all(ValidationTrigger::Submit);
        self.mark_unresolved_async_validators_pending_for_submit_intent(&intent);

        if self.has_submit_blocking_errors(&intent)
            || self.has_pending_validation_for_submit_intent(&intent)
        {
            let blocker = self.submit_validation_blocker_for_intent(&intent);
            self.record_submit_status(SubmitStatus::Blocked(blocker), intent);
            return SubmitAttempt::Blocked(blocker);
        }

        self.submission.set_in_flight(true);
        self.submission
            .set_in_flight_intent(Some(SubmitIntentSnapshot::new(intent.clone())));
        SubmitAttempt::Started(SubmissionSnapshot::with_intent_and_field_versions(
            self.snapshot(),
            intent,
            self.field_store.versions_cloned(),
        ))
    }

    /// Starts a submission after external async submit validation has completed for a snapshot.
    pub fn begin_submission_after_validation(
        &mut self,
        validation: &SubmitValidationSnapshot,
    ) -> SubmitAttempt<Model> {
        self.intent(())
            .begin_submission_after_validation(validation)
    }

    fn begin_intent_submission_after_validation<Intent>(
        &mut self,
        validation: &SubmitValidationSnapshot<Intent>,
    ) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + PartialEq + 'static,
    {
        if self.submission.is_in_flight() {
            self.record_submit_status(
                SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission),
                validation.intent.clone(),
            );
            return SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission);
        }

        if self.form_version != validation.form_version
            || self.field_store.versions() != &validation.field_versions
            || self.has_submit_blocking_errors(&validation.intent)
            || self.has_pending_validation_for_submit_intent(&validation.intent)
            || self.has_unresolved_async_validation_for_submit_intent(&validation.intent)
        {
            let blocker = self.submit_validation_blocker_for_intent(&validation.intent);
            self.record_submit_status(SubmitStatus::Blocked(blocker), validation.intent.clone());
            return SubmitAttempt::Blocked(blocker);
        }

        self.submission.set_in_flight(true);
        self.submission
            .set_validation_intent(Some(SubmitIntentSnapshot::new(validation.intent.clone())));
        self.submission
            .set_in_flight_intent(Some(SubmitIntentSnapshot::new(validation.intent.clone())));
        SubmitAttempt::Started(SubmissionSnapshot::with_intent_and_field_versions(
            self.snapshot(),
            validation.intent.clone(),
            self.field_store.versions_cloned(),
        ))
    }

    /// Records a duplicate submission attempt while adapter-managed validation is in flight.
    pub fn block_duplicate_submission(&mut self) -> SubmitAttempt<Model> {
        self.intent(()).block_duplicate_submission()
    }

    fn block_duplicate_intent_submission<Intent>(
        &mut self,
        intent: Intent,
    ) -> SubmitAttempt<Model, Intent>
    where
        Intent: 'static,
    {
        self.record_submit_status(
            SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission),
            intent,
        );
        SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission)
    }

    /// Returns the current field-version snapshot used for stale submit protection.
    pub fn submit_validation_field_versions(&self) -> BTreeMap<FieldIdentity, u64> {
        self.field_store.versions_cloned()
    }

    /// Returns the current validation lifecycle snapshot used for stale submit protection.
    pub fn submit_validation_snapshot(&self) -> SubmitValidationSnapshot {
        SubmitValidationSnapshot::new(self.form_version, self.field_store.versions_cloned(), ())
    }

    /// Returns the current validation lifecycle snapshot for an explicit submit intent.
    pub fn intent_validation_snapshot<Intent>(
        &self,
        intent: Intent,
    ) -> SubmitValidationSnapshot<Intent> {
        SubmitValidationSnapshot::new(
            self.form_version,
            self.field_store.versions_cloned(),
            intent,
        )
    }

    /// Records a parse-blocked submission attempt from a renderer adapter.
    pub fn block_submission_with_parse_errors(&mut self) -> SubmitAttempt<Model> {
        self.intent(()).block_submission_with_parse_errors()
    }

    fn block_intent_submission_with_parse_errors<Intent>(
        &mut self,
        intent: Intent,
    ) -> SubmitAttempt<Model, Intent>
    where
        Intent: 'static,
    {
        if self.submission.is_in_flight() {
            self.record_submit_status(
                SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission),
                intent,
            );
            return SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission);
        }

        self.mark_submit_attempt();
        self.record_submit_status(SubmitStatus::Blocked(SubmitBlocker::ParseErrors), intent);
        SubmitAttempt::Blocked(SubmitBlocker::ParseErrors)
    }

    /// Records parse blocking for an already-counted adapter-managed submit attempt.
    pub fn block_submission_with_parse_errors_after_validation(&mut self) -> SubmitAttempt<Model> {
        self.intent(())
            .block_submission_with_parse_errors_after_validation()
    }

    fn block_intent_submission_with_parse_errors_after_validation<Intent>(
        &mut self,
        intent: Intent,
    ) -> SubmitAttempt<Model, Intent>
    where
        Intent: 'static,
    {
        if self.submission.is_in_flight() {
            self.record_submit_status(
                SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission),
                intent,
            );
            return SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission);
        }

        self.record_submit_status(SubmitStatus::Blocked(SubmitBlocker::ParseErrors), intent);
        SubmitAttempt::Blocked(SubmitBlocker::ParseErrors)
    }

    /// Runs a synchronous submit handler when submit validation passes.
    pub fn submit<Submit, Outcome>(&mut self, submit: Submit) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        self.intent(()).submit(submit)
    }

    fn submit_intent<Intent, Submit, Outcome>(
        &mut self,
        intent: Intent,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        match self.begin_intent_submission(intent) {
            SubmitAttempt::Started(submitted) => {
                let submitted_for_result = submitted.clone();
                let submit_errors = submit(submitted).into();

                if submit_errors.is_empty() {
                    self.finish_submission_success();
                    SubmitResult::Succeeded
                } else {
                    self.finish_submission_with_errors(submitted_for_result, submit_errors);
                    SubmitResult::Rejected
                }
            }
            SubmitAttempt::Blocked(blocker) => SubmitResult::Blocked(blocker),
        }
    }

    /// Restores the current draft to the baseline and clears interaction and validation state.
    pub fn reset(&mut self) {
        self.draft.reset();
        self.increment_form_version();
        self.field_store.clear();
        self.validation_chains
            .clear_collection_item_field_validator_states();
        self.clear_validation_results();
        self.emit_observer_event(FormObserverEvent::Reset {
            value: FormObserverValue::Redacted,
        });
    }

    /// Explicitly replaces the baseline and current draft, clearing interaction and validation state.
    pub fn reinitialize(&mut self, initial: Model) {
        self.draft.reinitialize(initial);
        self.increment_form_version();
        self.field_store.clear();
        self.validation_chains
            .clear_collection_item_field_validator_states();
        self.clear_validation_results();
        self.emit_observer_event(FormObserverEvent::Reinitialized {
            value: FormObserverValue::Redacted,
        });
    }

    /// Restores one field to its current baseline value and clears that field's field-scoped state.
    ///
    /// The field value is reset to the current **Baseline Value** (so a reinitialized baseline is
    /// honored, not the original configuration value), the field's touched and blurred metadata is
    /// cleared (dirty is derived and becomes clean once the value matches the baseline), and that
    /// field's field-scoped validator results and pending validation are cleared. Other fields,
    /// form-level validators, and submit state for other fields are left untouched.
    pub fn reset_field<Value>(&mut self, path: FieldPath<Model, Value>)
    where
        Value: Clone,
    {
        let field = FormObserverField::from_path(&path);
        let field_identity = field.identity();

        let baseline = path.get(self.draft.baseline()).clone();
        *path.get_mut(self.draft.current_mut()) = baseline;

        *self.field_store.metadata_mut(&field_identity) = FieldMetadata::default();
        self.increment_form_version();
        self.increment_field_version(&field_identity);
        self.validation_chains.clear_field_results(&field_identity);
        self.invalidate_async_field_validators_for_model_change();
        self.invalidate_pending_async_form_validators();
        self.clear_submit_errors_for_field(&field_identity);
        self.emit_observer_event(FormObserverEvent::FieldReset { field });
    }
}

impl<Model, Error, Intent> FormCoreIntent<'_, Model, Error, Intent> {
    /// Returns the submit intent this scope uses.
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Returns the current validation lifecycle snapshot for this submit intent.
    pub fn validation_snapshot(&self) -> SubmitValidationSnapshot<Intent>
    where
        Intent: Clone,
    {
        SubmitValidationSnapshot::new(
            self.core.form_version,
            self.core.field_store.versions_cloned(),
            self.intent.clone(),
        )
    }

    /// Returns current UI-oriented submit availability for this submit intent.
    pub fn availability(&self) -> SubmitAvailability
    where
        Intent: PartialEq + 'static,
    {
        self.core.intent_availability(&self.intent)
    }

    /// Returns whether this submit intent has no current known blockers.
    pub fn can_submit(&self) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.availability().is_available()
    }

    /// Returns the latest outcome when this submit intent produced the latest status.
    pub fn last_status(&self) -> Option<SubmitStatus>
    where
        Intent: PartialEq + 'static,
    {
        self.core.intent_last_status(&self.intent)
    }

    /// Records a submit attempt and runs submit-triggered validators for this intent.
    pub fn validate_for_submit(&mut self) -> bool
    where
        Intent: Clone + PartialEq + 'static,
    {
        self.core.validate_intent_for_submit(self.intent.clone())
    }

    /// Runs submit-triggered synchronous validators for adapter-owned submit preflight.
    pub fn validate_for_submit_preflight(&mut self) -> Option<SubmitBlocker>
    where
        Intent: Clone + PartialEq + 'static,
    {
        self.core
            .validate_intent_for_submit_preflight(self.intent.clone())
    }
}

impl<Model: Clone, Error, Intent> FormCoreIntent<'_, Model, Error, Intent> {
    /// Starts a submission for this submit intent.
    pub fn begin_submission(&mut self) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + PartialEq + 'static,
    {
        self.core.begin_intent_submission(self.intent.clone())
    }

    /// Starts a submission after external async submit validation has completed for this intent.
    pub fn begin_submission_after_validation(
        &mut self,
        validation: &SubmitValidationSnapshot<Intent>,
    ) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + PartialEq + 'static,
    {
        assert!(
            validation.intent() == &self.intent,
            "submit validation snapshot intent does not match scoped submit intent"
        );

        self.core
            .begin_intent_submission_after_validation(validation)
    }

    /// Records a duplicate submission attempt for this submit intent.
    pub fn block_duplicate_submission(&mut self) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + 'static,
    {
        self.core
            .block_duplicate_intent_submission(self.intent.clone())
    }

    /// Records a parse-blocked submission attempt for this submit intent.
    pub fn block_submission_with_parse_errors(&mut self) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + 'static,
    {
        self.core
            .block_intent_submission_with_parse_errors(self.intent.clone())
    }

    /// Records parse blocking for an already-counted submit attempt for this intent.
    pub fn block_submission_with_parse_errors_after_validation(
        &mut self,
    ) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + 'static,
    {
        self.core
            .block_intent_submission_with_parse_errors_after_validation(self.intent.clone())
    }

    /// Runs a synchronous submit handler for this submit intent.
    pub fn submit<Submit, Outcome>(&mut self, submit: Submit) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        self.core.submit_intent(self.intent.clone(), submit)
    }
}

impl<Model, Error> FormCore<Model, Error> {
    /// Returns whether a field async validator is pending and relevant to submit validation.
    pub fn is_pending_submit_field_validator(&self, field: FieldIdentity, id: ValidatorId) -> bool {
        self.validation_chains
            .field_validator(&ValidatorKey::new(field, id))
            .is_some_and(|validator| {
                validator.lifecycle.is_async()
                    && validator
                        .lifecycle
                        .should_flush_debounced_async_for_submit()
            })
    }

    /// Returns whether a form async validator is pending and relevant to submit validation.
    pub fn is_pending_submit_form_validator(&self, id: ValidatorId) -> bool {
        self.validation_chains
            .form_validator(id)
            .is_some_and(|validator| {
                validator.lifecycle.is_async()
                    && validator
                        .lifecycle
                        .should_flush_debounced_async_for_submit()
            })
    }

    /// Returns whether a delayed async validation should be flushed for submit correctness.
    pub fn should_flush_debounced_validation_for_submit(
        &self,
        target: &ValidationTarget,
        id: ValidatorId,
    ) -> bool {
        match target {
            ValidationTarget::Field(field) => {
                self.is_pending_submit_field_validator(field.clone(), id)
            }
            ValidationTarget::Form => self.is_pending_submit_form_validator(id),
        }
    }

    /// Registers an observer for future form transition events.
    ///
    /// The core does not store event history or replay past events to new observers.
    pub fn observe<Observer>(&mut self, observer: Observer)
    where
        Observer: FnMut(&FormObserverEvent) + 'static,
    {
        self.observers.push(Box::new(observer));
    }

    /// Returns the owned form draft.
    pub fn draft(&self) -> &FormDraft<Model> {
        &self.draft
    }

    /// Captures opt-in form state for explicit serialization or transfer.
    ///
    /// The snapshot includes form draft values, field metadata, non-submit validation result state,
    /// and runtime collection item identity sequences. It does not include submit-scoped validation
    /// results, submit errors, observer registrations, validator closures, adapter parse bindings,
    /// async tasks, or in-flight submission futures.
    pub fn state_snapshot(&self) -> FormStateSnapshot<Model, Error>
    where
        Model: Clone,
        Error: Clone,
    {
        FormStateSnapshot {
            version: FORM_STATE_SERIALIZATION_VERSION,
            draft: self.draft.clone(),
            validation_mode: self.validation_mode,
            error_visibility_policy: self.error_visibility_policy,
            form_version: self.form_version,
            field_versions: self
                .field_store
                .iter_versions()
                .filter(|(field, _)| !field.is_file())
                .map(|(field, version)| FieldVersionSnapshot {
                    field: field.clone(),
                    version: *version,
                })
                .collect(),
            field_metadata: self
                .field_store
                .iter_metadata()
                .filter(|(field, _)| !field.is_file())
                .map(|(field, metadata)| FieldMetadataSnapshot {
                    field: field.clone(),
                    metadata: *metadata,
                })
                .collect(),
            collection_identities: CollectionIdentityState::from_collection_states(
                self.field_store.collections(),
            ),
            field_validator_states: self
                .validation_chains
                .field_entries()
                .filter(|(key, _)| !key.field.is_file())
                .filter(|(_, validator)| !validator.lifecycle.has_submit_scoped_status())
                .map(|(key, validator)| FieldValidatorStateSnapshot {
                    field: key.field.clone(),
                    validator_id: key.id,
                    state: validator.lifecycle.snapshot_result(),
                })
                .collect(),
            collection_item_validator_states: self
                .validation_chains
                .collection_item_state_entries()
                .filter(|(_, state)| !state.has_submit_scoped_status())
                .map(|(key, state)| FieldValidatorStateSnapshot {
                    field: key.field.clone(),
                    validator_id: key.id,
                    state: state.snapshot_result(),
                })
                .collect(),
            form_validator_states: self
                .validation_chains
                .form_entries()
                .filter(|(_, validator)| !validator.lifecycle.has_submit_scoped_status())
                .map(|(validator_id, validator)| FormValidatorStateSnapshot {
                    validator_id: *validator_id,
                    state: without_file_targeted_form_validation_errors(
                        validator.lifecycle.snapshot_result(),
                    ),
                })
                .collect(),
            next_validator_id: self.validation_chains.next_validator_id(),
            submit_attempts: self.submission.attempt_count(),
        }
    }

    /// Restores an opt-in form-state snapshot onto this configured form core.
    ///
    /// Existing observers and validator registrations stay in place. Stored validator results are
    /// restored for matching validator IDs, while validator source labels, triggers, sync/async
    /// kind, and behavior stay defined by the normal form configuration. Pending async work,
    /// submit-validation intent, and pre-restore submit-validation freshness tokens are not
    /// restored.
    pub fn restore_state_snapshot(
        &mut self,
        snapshot: FormStateSnapshot<Model, Error>,
    ) -> Result<(), FormStateRestoreError> {
        if snapshot.version != FORM_STATE_SERIALIZATION_VERSION {
            return Err(FormStateRestoreError::UnsupportedFormStateVersion {
                expected: FORM_STATE_SERIALIZATION_VERSION,
                actual: snapshot.version,
            });
        }

        let collection_states = snapshot.collection_identities.into_collection_states()?;
        let field_versions = snapshot
            .field_versions
            .into_iter()
            .filter(|entry| !entry.field.is_file())
            .map(|entry| (entry.field, entry.version))
            .collect();
        let field_metadata = snapshot
            .field_metadata
            .into_iter()
            .filter(|entry| !entry.field.is_file())
            .map(|entry| (entry.field, entry.metadata))
            .collect();

        let restored_form_version = self
            .form_version
            .max(snapshot.form_version)
            .saturating_add(1);

        self.draft = snapshot.draft;
        self.validation_mode = snapshot.validation_mode;
        self.error_visibility_policy = snapshot.error_visibility_policy;
        self.form_version = restored_form_version;
        self.field_store
            .restore_fields(field_versions, field_metadata);
        self.field_store.set_collections(collection_states);

        for validator in self.validation_chains.field_values_mut() {
            validator.lifecycle.clear();
        }

        for validator in self.validation_chains.form_values_mut() {
            validator.lifecycle.clear();
        }

        for validator_state in snapshot.field_validator_states {
            if validator_state.field.is_file() {
                continue;
            }

            if let Some(validator) = self
                .validation_chains
                .field_validator_mut(&validator_state.key())
            {
                validator
                    .lifecycle
                    .restore_result_from_snapshot(validator_state.state);
            }
        }

        self.validation_chains
            .clear_collection_item_field_validator_states();
        self.ensure_all_collection_item_validator_states();

        for validator_state in snapshot.collection_item_validator_states {
            if let Some(validator) = self
                .validation_chains
                .collection_item_state_mut(&validator_state.key())
            {
                validator.restore_result_from_snapshot(validator_state.state);
            }
        }

        for mut validator_state in snapshot.form_validator_states {
            validator_state.state =
                without_file_targeted_form_validation_errors(validator_state.state);

            if let Some(validator) = self
                .validation_chains
                .form_validator_mut(validator_state.validator_id)
            {
                validator
                    .lifecycle
                    .restore_result_from_snapshot(validator_state.state);
            }
        }

        self.validation_chains
            .advance_next_validator_id_to_at_least(snapshot.next_validator_id);
        self.submission.reset();
        self.submission
            .restore_attempt_count(snapshot.submit_attempts);

        Ok(())
    }

    /// Returns serializable runtime identity state for all tracked collection fields.
    pub fn collection_identity_state(&self) -> CollectionIdentityState {
        CollectionIdentityState::from_collection_states(self.field_store.collections())
    }

    /// Restores serializable runtime identity state for tracked collection fields.
    pub fn restore_collection_identity_state(
        &mut self,
        state: CollectionIdentityState,
    ) -> Result<(), FormStateRestoreError> {
        self.field_store
            .set_collections(state.into_collection_states()?);
        Ok(())
    }

    /// Returns the mode that controls automatic validation execution.
    pub const fn validation_mode(&self) -> ValidationMode {
        self.validation_mode
    }

    /// Replaces the mode that controls automatic validation execution.
    pub fn set_validation_mode(&mut self, mode: ValidationMode) {
        self.validation_mode = mode;
    }

    /// Returns this form core with a mode for automatic validation execution.
    pub fn with_validation_mode(mut self, mode: ValidationMode) -> Self {
        self.set_validation_mode(mode);
        self
    }

    /// Returns the policy that controls visible validation errors.
    pub const fn error_visibility_policy(&self) -> ErrorVisibilityPolicy {
        self.error_visibility_policy
    }

    /// Replaces the policy that controls visible validation errors.
    pub fn set_error_visibility_policy(&mut self, policy: ErrorVisibilityPolicy) {
        self.error_visibility_policy = policy;
    }

    /// Returns this form core with a visible-error policy.
    pub fn with_error_visibility_policy(mut self, policy: ErrorVisibilityPolicy) -> Self {
        self.set_error_visibility_policy(policy);
        self
    }

    /// Reads a typed field value from the current draft.
    pub fn field_value<Value>(&self, path: FieldPath<Model, Value>) -> &Value {
        path.get(self.draft.current())
    }

    /// Returns whether a submission has started and not completed yet.
    pub const fn is_submitting(&self) -> bool {
        self.submission.is_in_flight()
    }

    /// Returns the latest meaningful submission outcome, if one has been recorded.
    pub fn last_submit_status(&self) -> Option<SubmitStatus> {
        self.submission.last_status().map(|status| status.status)
    }

    /// Returns whether the latest recorded submit outcome was a successful submission.
    ///
    /// A pure derived read over [`Self::last_submit_status`]. A form that has not completed a
    /// submit, or whose latest outcome was a rejection or a blocked attempt, reports `false`. It is
    /// cleared by [`Reset`](Self::reset) and reinitialization along with the rest of submit state.
    pub fn is_submit_successful(&self) -> bool {
        self.last_submit_status()
            .is_some_and(SubmitStatus::is_succeeded)
    }

    /// Returns the latest meaningful submission outcome with its typed submit intent.
    pub fn last_submit_status_as<Intent>(&self) -> Option<LastSubmitStatus<Intent>>
    where
        Intent: Clone + 'static,
    {
        self.submission.last_status()?.typed()
    }

    /// Returns the latest outcome for `intent` when that intent produced the latest status.
    pub fn intent_last_status<Intent>(&self, intent: &Intent) -> Option<SubmitStatus>
    where
        Intent: PartialEq + 'static,
    {
        let status = self.submission.last_status()?;

        if status.matches_intent(intent) {
            Some(status.status)
        } else {
            None
        }
    }

    /// Returns current UI-oriented submit availability.
    pub fn submit_availability(&self) -> SubmitAvailability {
        let mut blockers = Vec::new();

        if self.has_validation_errors() {
            blockers.push(SubmitBlocker::ValidationErrors);
        }

        if self.has_pending_validation_for_trigger(ValidationTrigger::Submit) {
            blockers.push(SubmitBlocker::PendingValidation);
        }

        if self.submission.is_in_flight() {
            blockers.push(SubmitBlocker::InFlightSubmission);
        }

        SubmitAvailability::blocked_by(blockers)
    }

    /// Returns current UI-oriented submit availability for a specific submit intent.
    ///
    /// This is a read-only known-blocker signal. Submission still performs submit-triggered
    /// validation with the provided intent before application submit behavior runs.
    pub fn intent_availability<Intent>(&self, intent: &Intent) -> SubmitAvailability
    where
        Intent: PartialEq + 'static,
    {
        let mut blockers = Vec::new();

        if self.has_known_submit_blocking_errors(intent) {
            blockers.push(SubmitBlocker::ValidationErrors);
        }

        if self.has_pending_validation_for_submit_intent(intent) {
            blockers.push(SubmitBlocker::PendingValidation);
        }

        if self.submission.is_in_flight() {
            blockers.push(SubmitBlocker::InFlightSubmission);
        }

        SubmitAvailability::blocked_by(blockers)
    }

    /// Returns whether there are no current known submit blockers.
    pub fn can_submit(&self) -> bool {
        self.submit_availability().is_available()
    }

    /// Records a blocked submit outcome for an attempt that has already been counted.
    pub fn record_submit_blocker_after_attempt<Intent>(
        &mut self,
        blocker: SubmitBlocker,
        intent: Intent,
    ) where
        Intent: 'static,
    {
        self.record_submit_status(SubmitStatus::Blocked(blocker), intent);
    }

    /// Completes an in-flight submission without changing values or the baseline.
    pub fn finish_submission(&mut self) -> bool {
        if !self.submission.is_in_flight() {
            return false;
        }

        self.submission.set_in_flight(false);
        self.submission.set_in_flight_intent(None);
        true
    }

    /// Completes a successful submission without resetting or changing the baseline.
    pub fn finish_submission_success(&mut self) -> bool {
        if !self.submission.is_in_flight() {
            return false;
        }

        self.submission.set_in_flight(false);
        self.clear_submit_errors();
        let intent = self.take_submission_in_flight_intent();
        self.record_submit_status_snapshot(SubmitStatus::Succeeded, intent);
        true
    }

    /// Completes an in-flight submission with structured submit errors.
    pub fn finish_submission_with_errors<Intent, Outcome>(
        &mut self,
        submitted: SubmissionSnapshot<Model, Intent>,
        errors: Outcome,
    ) -> bool
    where
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        if !self.submission.is_in_flight() {
            return false;
        }

        self.submission.set_in_flight(false);
        let intent = self.take_submission_in_flight_intent();
        self.store_submit_errors(&submitted, errors.into(), intent.clone());
        self.record_submit_status_snapshot(SubmitStatus::Rejected, intent);
        true
    }

    /// Returns whether any form value differs from the baseline value.
    pub fn is_dirty(&self) -> bool
    where
        Model: PartialEq,
    {
        self.draft.current() != self.draft.baseline()
    }

    /// Returns whether one field value differs from its baseline value.
    pub fn is_field_dirty<Value>(&self, path: FieldPath<Model, Value>) -> bool
    where
        Value: PartialEq,
    {
        path.get(self.draft.current()) != path.get(self.draft.baseline())
    }

    /// Returns whether every form value equals the baseline value.
    ///
    /// This is the inverse of [`Self::is_dirty`], provided for symmetry with UI code that reasons
    /// about pristine forms. It is a pure derived read over the same non-sticky baseline comparison.
    pub fn is_pristine(&self) -> bool
    where
        Model: PartialEq,
    {
        !self.is_dirty()
    }

    /// Returns whether one field currently equals its baseline value.
    ///
    /// Because [`Dirty Fields`](Self::is_field_dirty) are non-sticky (reverting a field to its
    /// baseline makes it clean again), this is the inverse of [`Self::is_field_dirty`] and reports
    /// whether the field holds its default/baseline value right now.
    pub fn is_default_value<Value>(&self, path: FieldPath<Model, Value>) -> bool
    where
        Value: PartialEq,
    {
        !self.is_field_dirty(path)
    }

    /// Returns whether any registered validator currently has a pending status.
    ///
    /// A pure derived read over [`Self::validation_statuses`]; it introduces no new stored state.
    pub fn is_validating(&self) -> bool {
        self.validation_statuses()
            .iter()
            .any(|status| status.status() == ValidationStatus::Pending)
    }

    /// Returns whether any validator attached to one field currently has a pending status.
    pub fn is_field_validating<Value>(&self, path: FieldPath<Model, Value>) -> bool {
        self.field_validation_statuses(path)
            .iter()
            .any(|status| status.status() == ValidationStatus::Pending)
    }

    /// Returns the current logical items for a direct `Vec<Item>` collection field.
    pub fn collection_items<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
    ) -> Vec<CollectionItem> {
        let items = self.ensure_collection_state(&path).items();
        self.ensure_collection_item_validator_states_for_collection(&path.identity());
        items
    }

    /// Returns whether the collection value or logical item order differs from its baseline.
    pub fn is_collection_dirty<Item>(&self, path: FieldPath<Model, Vec<Item>>) -> bool
    where
        Item: PartialEq,
    {
        self.field_store
            .collection(&path.identity())
            .map(CollectionState::is_dirty)
            .unwrap_or(false)
            || path.get(self.draft.current()) != path.get(self.draft.baseline())
    }

    /// Reads one child field from a logical collection item.
    pub fn collection_item_field_value<'a, Item: 'a, Value>(
        &'a self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> Option<&'a Value> {
        let index = self
            .field_store
            .collection(&collection.identity())?
            .current_index(item)?;
        collection
            .get(self.draft.current())
            .get(index)
            .map(|item| field.get(item))
    }

    /// Returns whether one child field inside a logical collection item differs from its baseline.
    pub fn is_collection_item_field_dirty<Item, Value>(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> bool
    where
        Value: PartialEq,
    {
        let Some(state) = self.field_store.collection(&collection.identity()) else {
            return false;
        };
        let Some(current_index) = state.current_index(item) else {
            return false;
        };
        let Some(baseline_index) = state.baseline_index(item) else {
            return true;
        };
        let Some(current_item) = collection.get(self.draft.current()).get(current_index) else {
            return false;
        };
        let Some(baseline_item) = collection.get(self.draft.baseline()).get(baseline_index) else {
            return true;
        };

        field.get(current_item) != field.get(baseline_item)
    }

    /// Inserts a collection item through the programmatic update path.
    pub fn insert_collection_item<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
    ) -> Option<CollectionItemIdentity> {
        self.insert_collection_item_with_origin(path, index, item, FieldUpdateOrigin::Programmatic)
    }

    /// Inserts a collection item through the user update path and marks the collection touched.
    pub fn insert_user_collection_item<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
    ) -> Option<CollectionItemIdentity> {
        self.insert_collection_item_with_origin(path, index, item, FieldUpdateOrigin::User)
    }

    /// Appends a collection item through the programmatic update path.
    pub fn push_collection_item<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        item: Item,
    ) -> CollectionItemIdentity {
        let index = path.get(self.draft.current()).len();

        self.insert_collection_item(path, index, item)
            .expect("append index should be valid")
    }

    /// Appends a collection item through the user update path and marks the collection touched.
    pub fn push_user_collection_item<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        item: Item,
    ) -> CollectionItemIdentity {
        let index = path.get(self.draft.current()).len();

        self.insert_user_collection_item(path, index, item)
            .expect("append index should be valid")
    }

    /// Removes a logical collection item through the programmatic update path.
    pub fn remove_collection_item<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
    ) -> Option<Item> {
        self.remove_collection_item_with_origin(path, item, FieldUpdateOrigin::Programmatic)
    }

    /// Removes a logical collection item through the user update path and marks the collection touched.
    pub fn remove_user_collection_item<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
    ) -> Option<Item> {
        self.remove_collection_item_with_origin(path, item, FieldUpdateOrigin::User)
    }

    /// Moves a logical collection item to a new index through the programmatic update path.
    pub fn move_collection_item_to_index<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        index: usize,
    ) -> bool {
        self.move_collection_item_to_index_with_origin(
            path,
            item,
            index,
            FieldUpdateOrigin::Programmatic,
        )
    }

    /// Moves a logical collection item to a new index through the user update path.
    pub fn move_user_collection_item_to_index<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        index: usize,
    ) -> bool {
        self.move_collection_item_to_index_with_origin(path, item, index, FieldUpdateOrigin::User)
    }

    /// Swaps two collection items by position through the programmatic update path.
    ///
    /// Each item keeps its library-owned **Collection Item Identity**, so item-scoped metadata,
    /// validation, parse state, and dirty tracking follow the swapped items. Returns `false` if
    /// either index is out of bounds or the two indices are equal (a no-op that changes nothing).
    pub fn swap_collection_items<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        a: usize,
        b: usize,
    ) -> bool {
        self.swap_collection_items_with_origin(path, a, b, FieldUpdateOrigin::Programmatic)
    }

    /// Swaps two collection items by position through the user update path.
    pub fn swap_user_collection_items<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        a: usize,
        b: usize,
    ) -> bool {
        self.swap_collection_items_with_origin(path, a, b, FieldUpdateOrigin::User)
    }

    /// Replaces one collection item's value in place through the programmatic update path.
    ///
    /// The existing item keeps its **Collection Item Identity**, so item-scoped metadata and
    /// validation stay attached. Returns `false` if the index is out of bounds.
    pub fn replace_collection_item<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
    ) -> bool {
        self.replace_collection_item_with_origin(path, index, item, FieldUpdateOrigin::Programmatic)
    }

    /// Replaces one collection item's value in place through the user update path.
    pub fn replace_user_collection_item<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
    ) -> bool {
        self.replace_collection_item_with_origin(path, index, item, FieldUpdateOrigin::User)
    }

    /// Removes every collection item through the programmatic update path.
    ///
    /// Releases item-scoped state for each removed item and returns their identities so the caller
    /// can release adapter-owned state. Other collections and form-level state are untouched.
    pub fn clear_collection_items<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
    ) -> Vec<CollectionItemIdentity> {
        self.clear_collection_items_with_origin(path, FieldUpdateOrigin::Programmatic)
    }

    /// Removes every collection item through the user update path.
    pub fn clear_user_collection_items<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
    ) -> Vec<CollectionItemIdentity> {
        self.clear_collection_items_with_origin(path, FieldUpdateOrigin::User)
    }

    /// Replaces one child field value inside a logical collection item programmatically.
    pub fn set_collection_item_field<Item, Value>(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
        value: Value,
    ) -> bool {
        self.replace_collection_item_field_with_origin(
            &collection,
            item,
            &field,
            value,
            FieldUpdateOrigin::Programmatic,
        )
    }

    /// Replaces one child field value inside a logical collection item because of user input.
    pub fn set_user_collection_item_field<Item, Value>(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
        value: Value,
    ) -> bool {
        let updated = self.replace_collection_item_field_with_origin(
            &collection,
            item,
            &field,
            value,
            FieldUpdateOrigin::User,
        );

        if updated {
            self.mark_collection_item_field_touched(collection, item, field);
        }

        updated
    }

    /// Marks one child field inside a logical collection item as touched.
    pub fn mark_collection_item_field_touched<Item, Value>(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> bool {
        if !self.collection_item_exists(&collection, item) {
            return false;
        }

        self.field_metadata_mut(&collection_item_field_identity(&collection, item, &field))
            .touched = true;
        true
    }

    /// Marks one child field inside a logical collection item as blurred and touched.
    pub fn mark_collection_item_field_blurred<Item, Value>(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> bool {
        if !self.collection_item_exists(&collection, item) {
            return false;
        }

        let metadata =
            self.field_metadata_mut(&collection_item_field_identity(&collection, item, &field));
        metadata.touched = true;
        metadata.blurred = true;

        if self
            .validation_mode
            .should_validate_on_blur(self.submission.attempt_count())
        {
            self.validate_collection_item_field_blur(collection, item, field);
        }

        true
    }

    /// Marks one child field inside a logical collection item as blurred without running validation.
    pub fn mark_collection_item_field_blurred_without_validation<Item, Value>(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> bool {
        if !self.collection_item_exists(&collection, item) {
            return false;
        }

        let metadata =
            self.field_metadata_mut(&collection_item_field_identity(&collection, item, &field));
        metadata.touched = true;
        metadata.blurred = true;
        true
    }

    /// Returns tracked user interaction metadata for one child field inside a logical collection item.
    pub fn collection_item_field_metadata<Item, Value>(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> FieldMetadata {
        self.field_store
            .metadata(&collection_item_field_identity(&collection, item, &field))
    }

    /// Returns tracked user interaction metadata for one field.
    pub fn field_metadata<Value>(&self, path: FieldPath<Model, Value>) -> FieldMetadata {
        self.field_store.metadata(&path.identity())
    }

    /// Returns tracked user interaction metadata for one field identity.
    pub fn field_metadata_by_identity(&self, field: &FieldIdentity) -> FieldMetadata {
        self.field_store.metadata(field)
    }

    /// Returns whether one field has received user interaction.
    pub fn is_field_touched<Value>(&self, path: FieldPath<Model, Value>) -> bool {
        self.field_metadata(path).is_touched()
    }

    /// Returns whether one field has lost focus at least once.
    pub fn is_field_blurred<Value>(&self, path: FieldPath<Model, Value>) -> bool {
        self.field_metadata(path).is_blurred()
    }

    /// Returns whether a field identity has received user interaction.
    pub fn is_field_identity_touched(&self, field: &FieldIdentity) -> bool {
        self.field_metadata_by_identity(field).is_touched()
    }

    /// Returns whether a field identity has lost focus at least once.
    pub fn is_field_identity_blurred(&self, field: &FieldIdentity) -> bool {
        self.field_metadata_by_identity(field).is_blurred()
    }

    /// Registers a synchronous validator for one direct field and every trigger.
    pub fn register_sync_field_validator<Value, Source, Validator>(
        &mut self,
        path: FieldPath<Model, Value>,
        source: Source,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Value: 'static,
    {
        self.register_sync_field_validator_for_triggers(
            path,
            source,
            ValidationTriggers::all(),
            validator,
        )
    }

    /// Registers a synchronous validator for one direct field and trigger set.
    pub fn register_sync_field_validator_for_triggers<Value, Source, Triggers, Validator>(
        &mut self,
        path: FieldPath<Model, Value>,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Value: 'static,
    {
        let id = self.allocate_validator_id();
        let key = ValidatorKey::new(path.identity(), id);
        let validate = move |model: &Model, context: ValidatorContext<'_, Model>| {
            validator(path.get(model), context)
        };

        self.validation_chains.insert_field_validator(
            key,
            RegisteredFieldValidator {
                lifecycle: validation_lifecycle::SourceState::new(
                    source.into(),
                    triggers.into(),
                    validation_lifecycle::SourceKind::Sync,
                ),
                validate: Some(Rc::new(validate)),
                model_dependent: true,
            },
        );

        id
    }

    /// Registers a synchronous validator for a field identity outside typed field-path access.
    pub fn register_sync_field_identity_validator_for_triggers<Source, Triggers, Validator>(
        &mut self,
        field: FieldIdentity,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(&'a Model, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
    {
        let id = self.allocate_validator_id();
        let key = ValidatorKey::new(field, id);

        self.validation_chains.insert_field_validator(
            key,
            RegisteredFieldValidator {
                lifecycle: validation_lifecycle::SourceState::new(
                    source.into(),
                    triggers.into(),
                    validation_lifecycle::SourceKind::Sync,
                ),
                validate: Some(Rc::new(validator)),
                model_dependent: true,
            },
        );

        id
    }

    /// Registers an asynchronous validator source for one direct field and trigger set.
    pub fn register_async_field_validator_for_triggers<Value, Source, Triggers>(
        &mut self,
        path: FieldPath<Model, Value>,
        source: Source,
        triggers: Triggers,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
    {
        let id = self.allocate_validator_id();
        let key = ValidatorKey::new(path.identity(), id);

        self.validation_chains.insert_field_validator(
            key,
            RegisteredFieldValidator {
                lifecycle: validation_lifecycle::SourceState::new(
                    source.into(),
                    triggers.into(),
                    validation_lifecycle::SourceKind::Async,
                ),
                validate: None,
                model_dependent: true,
            },
        );

        id
    }

    /// Registers an asynchronous validator source for a field identity outside typed field-path access.
    pub fn register_async_field_identity_validator_for_triggers<Source, Triggers>(
        &mut self,
        field: FieldIdentity,
        source: Source,
        triggers: Triggers,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
    {
        self.register_async_field_identity_validator_for_triggers_with_model_dependency(
            field, source, triggers, false,
        )
    }

    /// Registers an asynchronous validator source for a field identity with explicit model context dependency.
    pub fn register_async_field_identity_validator_for_triggers_with_model_dependency<
        Source,
        Triggers,
    >(
        &mut self,
        field: FieldIdentity,
        source: Source,
        triggers: Triggers,
        model_dependent: bool,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
    {
        let id = self.allocate_validator_id();
        let key = ValidatorKey::new(field, id);

        self.validation_chains.insert_field_validator(
            key,
            RegisteredFieldValidator {
                lifecycle: validation_lifecycle::SourceState::new(
                    source.into(),
                    triggers.into(),
                    validation_lifecycle::SourceKind::Async,
                ),
                validate: None,
                model_dependent,
            },
        );

        id
    }

    /// Registers a zero-or-one-error synchronous validator for one direct field and every trigger.
    pub fn register_sync_field_validator_optional<Value, Source, Validator>(
        &mut self,
        path: FieldPath<Model, Value>,
        source: Source,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Option<Error> + 'static,
        Model: 'static,
        Value: 'static,
    {
        self.register_sync_field_validator_optional_for_triggers(
            path,
            source,
            ValidationTriggers::all(),
            validator,
        )
    }

    /// Registers a zero-or-one-error synchronous validator for one direct field and trigger set.
    pub fn register_sync_field_validator_optional_for_triggers<Value, Source, Triggers, Validator>(
        &mut self,
        path: FieldPath<Model, Value>,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Option<Error> + 'static,
        Model: 'static,
        Value: 'static,
    {
        self.register_sync_field_validator_for_triggers(
            path,
            source,
            triggers,
            move |value, context| validator(value, context).into_iter().collect(),
        )
    }

    /// Registers a synchronous validator template for one child field on every current and future
    /// item of a direct `Vec<Item>` collection field.
    pub fn register_sync_collection_item_field_validator<Item, Value, Source, Validator>(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        field: FieldPath<Item, Value>,
        source: Source,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Item: 'static,
        Value: 'static,
    {
        self.register_sync_collection_item_field_validator_for_triggers(
            collection,
            field,
            source,
            ValidationTriggers::all(),
            validator,
        )
    }

    /// Registers a synchronous validator template for one collection item child field and trigger set.
    pub fn register_sync_collection_item_field_validator_for_triggers<
        Item,
        Value,
        Source,
        Triggers,
        Validator,
    >(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        field: FieldPath<Item, Value>,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Item: 'static,
        Value: 'static,
    {
        let id = self.allocate_validator_id();
        let collection_identity = collection.identity();
        let field_identity_value = field.identity();
        let field_identity = field_identity_value
            .as_static_path()
            .expect("collection item field validator templates require direct static item fields");
        let collection_for_validate = collection.clone();
        let collection_for_len = collection.clone();
        let field_for_validate = field.clone();
        let validate = move |model: &Model,
                             collection_state: &CollectionState,
                             item: CollectionItemIdentity,
                             context: ValidatorContext<'_, Model>| {
            let Some(index) = collection_state.current_index(item) else {
                return Vec::new();
            };
            let Some(item_value) = collection_for_validate.get(model).get(index) else {
                return Vec::new();
            };

            validator(field_for_validate.get(item_value), context)
        };
        let key = CollectionItemValidatorTemplateKey::new(
            collection_identity.clone(),
            field_identity,
            id,
        );

        self.ensure_collection_state(&collection);
        self.validation_chains.insert_collection_item_template(
            key,
            RegisteredCollectionItemFieldValidator {
                source: source.into(),
                triggers: triggers.into(),
                collection_len: Rc::new(move |model| collection_for_len.get(model).len()),
                validate: Rc::new(validate),
            },
        );
        self.ensure_collection_item_validator_states_for_collection(&collection_identity);

        id
    }

    /// Registers a zero-or-one-error synchronous validator template for one collection item child field.
    pub fn register_sync_collection_item_field_validator_optional<Item, Value, Source, Validator>(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        field: FieldPath<Item, Value>,
        source: Source,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Option<Error> + 'static,
        Model: 'static,
        Item: 'static,
        Value: 'static,
    {
        self.register_sync_collection_item_field_validator_for_triggers(
            collection,
            field,
            source,
            ValidationTriggers::all(),
            move |value, context| validator(value, context).into_iter().collect(),
        )
    }

    /// Removes one field validator by stable ID and clears its validation result.
    pub fn unregister_field_validator_by_id<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
    ) -> bool {
        self.validation_chains
            .remove_field_validator(&ValidatorKey::new(path.identity(), id))
    }

    /// Removes the first field validator with this source label and clears its validation result.
    pub fn unregister_field_validator<Value, Source>(
        &mut self,
        path: FieldPath<Model, Value>,
        source: Source,
    ) -> bool
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let Some(key) = self.field_validator_key_for_source(path.identity(), &source) else {
            return false;
        };

        self.validation_chains.remove_field_validator(&key)
    }

    /// Registers a synchronous validator for the whole form and every trigger.
    pub fn register_sync_form_validator<Source, Validator>(
        &mut self,
        source: Source,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Vec<FormValidationError<Error>>
            + 'static,
        Model: 'static,
    {
        self.register_sync_form_validator_for_triggers(source, ValidationTriggers::all(), validator)
    }

    /// Registers a synchronous validator for the whole form and trigger set.
    pub fn register_sync_form_validator_for_triggers<Source, Triggers, Validator>(
        &mut self,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Vec<FormValidationError<Error>>
            + 'static,
        Model: 'static,
    {
        let id = self.allocate_validator_id();

        self.validation_chains.insert_form_validator(
            id,
            RegisteredFormValidator {
                lifecycle: validation_lifecycle::SourceState::new(
                    source.into(),
                    triggers.into(),
                    validation_lifecycle::SourceKind::Sync,
                ),
                validate: Some(Box::new(validator)),
            },
        );

        id
    }

    /// Registers an asynchronous validator source for the whole form and trigger set.
    pub fn register_async_form_validator_for_triggers<Source, Triggers>(
        &mut self,
        source: Source,
        triggers: Triggers,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
    {
        let id = self.allocate_validator_id();

        self.validation_chains.insert_form_validator(
            id,
            RegisteredFormValidator {
                lifecycle: validation_lifecycle::SourceState::new(
                    source.into(),
                    triggers.into(),
                    validation_lifecycle::SourceKind::Async,
                ),
                validate: None,
            },
        );

        id
    }

    /// Registers a zero-or-one-error synchronous validator for the whole form and every trigger.
    pub fn register_sync_form_validator_optional<Source, Validator>(
        &mut self,
        source: Source,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Option<FormValidationError<Error>>
            + 'static,
        Model: 'static,
    {
        self.register_sync_form_validator_optional_for_triggers(
            source,
            ValidationTriggers::all(),
            validator,
        )
    }

    /// Registers a zero-or-one-error synchronous validator for the whole form and trigger set.
    pub fn register_sync_form_validator_optional_for_triggers<Source, Triggers, Validator>(
        &mut self,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Option<FormValidationError<Error>>
            + 'static,
        Model: 'static,
    {
        self.register_sync_form_validator_for_triggers(source, triggers, move |context| {
            validator(context).into_iter().collect()
        })
    }

    /// Removes one form validator by stable ID and clears its validation result.
    pub fn unregister_form_validator_by_id(&mut self, id: ValidatorId) -> bool {
        self.validation_chains.remove_form_validator(id)
    }

    /// Removes the first form validator with this source label and clears its validation result.
    pub fn unregister_form_validator<Source>(&mut self, source: Source) -> bool
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let Some(id) = self.form_validator_id_for_source(&source) else {
            return false;
        };

        self.validation_chains.remove_form_validator(id)
    }

    /// Runs validators registered for one field and trigger, then form validators for the same trigger.
    pub fn validate_field<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        trigger: ValidationTrigger,
    ) {
        self.validate_field_chain(&path.identity(), trigger);
        self.validate_form_chain(trigger);
    }

    /// Runs one validator registered for one field and trigger.
    ///
    /// Returns `None` when the ID is missing, belongs to another field, or is not registered for `trigger`.
    pub fn validate_field_validator<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus> {
        self.validate_field_validator_key(ValidatorKey::new(path.identity(), id), trigger)
    }

    /// Runs the first validator source label registered for one field and trigger.
    ///
    /// Returns `None` when the source is missing or is not registered for `trigger`.
    pub fn validate_field_source<Value, Source>(
        &mut self,
        path: FieldPath<Model, Value>,
        source: Source,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus>
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let key = self.field_validator_key_for_source(path.identity(), &source)?;

        self.validate_field_validator_key(key, trigger)
    }

    /// Starts an asynchronous field validation run and marks the validator source pending.
    pub fn begin_async_field_validation<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFieldValidation<Model, Value>>
    where
        Model: Clone,
        Value: Clone,
    {
        let key = ValidatorKey::new(path.identity(), id);
        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync() || !validator.lifecycle.should_run(trigger) {
            return None;
        }

        self.with_async_start_sync_gate(
            ValidationTarget::Field(key.field.clone()),
            trigger,
            |core| core.begin_async_field_validation_after_sync(path, id, trigger),
        )
    }

    /// Starts an asynchronous field validator after its sync chain has already run.
    pub fn begin_async_field_validation_after_sync<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFieldValidation<Model, Value>>
    where
        Model: Clone,
        Value: Clone,
    {
        let key = ValidatorKey::new(path.identity(), id);
        let validator = self.validation_chains.field_validator(&key)?;

        if !validator.lifecycle.should_begin_async_after_sync(trigger) {
            return None;
        }

        let form = self.draft.current().clone();
        let field_value = path.get(self.draft.current()).clone();
        let form_version = self.form_version;
        let field_version = self.current_field_version(&key.field);
        let submit_intent = self.submit_intent_for_trigger(trigger);
        let (source, run_id, outcome) = {
            let validator = self
                .validation_chains
                .field_validator_mut(&key)
                .expect("validator disappeared during async validation scheduling");
            let (run_id, outcome) = validator
                .lifecycle
                .mark_async_started(trigger, submit_intent.clone());
            (outcome.source().clone(), run_id, outcome)
        };

        self.emit_lifecycle_observer_event(ValidationTarget::Field(key.field.clone()), outcome);

        Some(AsyncFieldValidation {
            form: FormSnapshot::new(form),
            field_value,
            field: key.field,
            validator_id: id,
            source,
            trigger,
            submit_intent,
            form_version,
            field_version,
            run_id,
        })
    }

    /// Starts an asynchronous field-identity validator and marks the source pending.
    pub fn begin_async_field_identity_validation(
        &mut self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFieldValidation<Model, ()>>
    where
        Model: Clone,
    {
        let key = ValidatorKey::new(field.clone(), id);
        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync() || !validator.lifecycle.should_run(trigger) {
            return None;
        }

        self.with_async_start_sync_gate(
            ValidationTarget::Field(key.field.clone()),
            trigger,
            |core| core.begin_async_field_identity_validation_after_sync(field, id, trigger),
        )
    }

    /// Starts an asynchronous field-identity validator after its sync chain has already run.
    pub fn begin_async_field_identity_validation_after_sync(
        &mut self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFieldValidation<Model, ()>>
    where
        Model: Clone,
    {
        let key = ValidatorKey::new(field, id);
        let validator = self.validation_chains.field_validator(&key)?;

        if !validator.lifecycle.should_begin_async_after_sync(trigger) {
            return None;
        }

        let form = self.draft.current().clone();
        let form_version = self.form_version;
        let field_version = self.current_field_version(&key.field);
        let submit_intent = self.submit_intent_for_trigger(trigger);
        let (source, run_id, outcome) = {
            let validator = self
                .validation_chains
                .field_validator_mut(&key)
                .expect("validator disappeared during async validation scheduling");
            let (run_id, outcome) = validator
                .lifecycle
                .mark_async_started(trigger, submit_intent.clone());
            (outcome.source().clone(), run_id, outcome)
        };

        self.emit_lifecycle_observer_event(ValidationTarget::Field(key.field.clone()), outcome);

        Some(AsyncFieldValidation {
            form: FormSnapshot::new(form),
            field_value: (),
            field: key.field,
            validator_id: id,
            source,
            trigger,
            submit_intent,
            form_version,
            field_version,
            run_id,
        })
    }

    /// Marks an asynchronous field validator pending before a delayed value-change run starts.
    pub fn schedule_debounced_async_field_validation<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<DebouncedAsyncFieldValidation> {
        let key = ValidatorKey::new(path.identity(), id);
        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync()
            || !validator.lifecycle.should_schedule_debounced_async(trigger)
        {
            return None;
        }

        self.with_async_start_sync_gate(
            ValidationTarget::Field(key.field.clone()),
            trigger,
            |core| core.schedule_debounced_async_field_validation_after_sync(path, id, trigger),
        )
    }

    /// Marks an asynchronous field validator pending after its sync chain has already run.
    pub fn schedule_debounced_async_field_validation_after_sync<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<DebouncedAsyncFieldValidation> {
        let key = ValidatorKey::new(path.identity(), id);
        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_schedule_debounced_async_after_sync(trigger)
        {
            return None;
        }

        let submit_intent = self.submit_intent_for_trigger(trigger);
        let (source, run_id, outcome) = {
            let validator = self
                .validation_chains
                .field_validator_mut(&key)
                .expect("validator disappeared during debounced validation scheduling");
            let (run_id, outcome) = validator
                .lifecycle
                .mark_debounced_async_pending(trigger, submit_intent);
            (outcome.source().clone(), run_id, outcome)
        };

        self.emit_lifecycle_observer_event(ValidationTarget::Field(key.field.clone()), outcome);

        Some(DebouncedAsyncFieldValidation {
            field: key.field,
            validator_id: id,
            source,
            trigger,
            run_id,
        })
    }

    /// Marks a field-identity async validator pending before a delayed value-change run starts.
    pub fn schedule_debounced_async_field_identity_validation(
        &mut self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<DebouncedAsyncFieldValidation> {
        let key = ValidatorKey::new(field.clone(), id);
        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync()
            || !validator.lifecycle.should_schedule_debounced_async(trigger)
        {
            return None;
        }

        self.with_async_start_sync_gate(
            ValidationTarget::Field(key.field.clone()),
            trigger,
            |core| {
                core.schedule_debounced_async_field_identity_validation_after_sync(
                    field, id, trigger,
                )
            },
        )
    }

    /// Marks a field-identity async validator pending after its sync chain has already run.
    pub fn schedule_debounced_async_field_identity_validation_after_sync(
        &mut self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<DebouncedAsyncFieldValidation> {
        let key = ValidatorKey::new(field, id);
        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_schedule_debounced_async_after_sync(trigger)
        {
            return None;
        }

        let submit_intent = self.submit_intent_for_trigger(trigger);
        let (source, run_id, outcome) = {
            let validator = self
                .validation_chains
                .field_validator_mut(&key)
                .expect("validator disappeared during debounced validation scheduling");
            let (run_id, outcome) = validator
                .lifecycle
                .mark_debounced_async_pending(trigger, submit_intent);
            (outcome.source().clone(), run_id, outcome)
        };

        self.emit_lifecycle_observer_event(ValidationTarget::Field(key.field.clone()), outcome);

        Some(DebouncedAsyncFieldValidation {
            field: key.field,
            validator_id: id,
            source,
            trigger,
            run_id,
        })
    }

    /// Starts the latest still-current delayed asynchronous field validation run.
    pub fn begin_debounced_async_field_validation<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        scheduled: &DebouncedAsyncFieldValidation,
    ) -> Option<AsyncFieldValidation<Model, Value>>
    where
        Model: Clone,
        Value: Clone,
    {
        let key = ValidatorKey::new(path.identity(), id);
        let scheduled_run = scheduled.lifecycle_run();
        let target = ValidationTarget::Field(key.field.clone());

        if !scheduled_run.matches_validator(&target, id) {
            return None;
        }

        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_begin_debounced_async(scheduled_run.trigger, scheduled_run.run_id)
        {
            return None;
        }

        let form = self.draft.current().clone();
        let field_value = path.get(self.draft.current()).clone();
        let form_version = self.form_version;
        let field_version = self.current_field_version(&key.field);
        let (run_id, outcome) = {
            let validator = self
                .validation_chains
                .field_validator_mut(&key)
                .expect("validator disappeared during debounced validation start");
            validator
                .lifecycle
                .begin_debounced_async(scheduled_run.trigger)
        };

        self.emit_lifecycle_observer_event(target, outcome);

        Some(AsyncFieldValidation {
            form: FormSnapshot::new(form),
            field_value,
            field: key.field,
            validator_id: id,
            source: scheduled.source.clone(),
            trigger: scheduled_run.trigger,
            submit_intent: self.submit_intent_for_trigger(scheduled_run.trigger),
            form_version,
            field_version,
            run_id,
        })
    }

    /// Starts the latest still-current delayed field-identity async validation run.
    pub fn begin_debounced_async_field_identity_validation(
        &mut self,
        field: FieldIdentity,
        id: ValidatorId,
        scheduled: &DebouncedAsyncFieldValidation,
    ) -> Option<AsyncFieldValidation<Model, ()>>
    where
        Model: Clone,
    {
        let key = ValidatorKey::new(field, id);
        let scheduled_run = scheduled.lifecycle_run();
        let target = ValidationTarget::Field(key.field.clone());

        if !scheduled_run.matches_validator(&target, id) {
            return None;
        }

        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_begin_debounced_async(scheduled_run.trigger, scheduled_run.run_id)
        {
            return None;
        }

        let form = self.draft.current().clone();
        let form_version = self.form_version;
        let field_version = self.current_field_version(&key.field);
        let (run_id, outcome) = {
            let validator = self
                .validation_chains
                .field_validator_mut(&key)
                .expect("validator disappeared during debounced validation start");
            validator
                .lifecycle
                .begin_debounced_async(scheduled_run.trigger)
        };

        self.emit_lifecycle_observer_event(target, outcome);

        Some(AsyncFieldValidation {
            form: FormSnapshot::new(form),
            field_value: (),
            field: key.field,
            validator_id: id,
            source: scheduled.source.clone(),
            trigger: scheduled_run.trigger,
            submit_intent: self.submit_intent_for_trigger(scheduled_run.trigger),
            form_version,
            field_version,
            run_id,
        })
    }

    /// Flushes a still-current delayed field validation for a specific trigger.
    ///
    /// This is used when submit needs to turn a pending value-change debounce into submit-scoped
    /// validation without waiting for the original debounce timer. When the flush changes the
    /// trigger, the target trigger's sync chain runs before async work starts.
    pub fn flush_debounced_async_field_validation_for_trigger<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        scheduled: &DebouncedAsyncFieldValidation,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFieldValidation<Model, Value>>
    where
        Model: Clone,
        Value: Clone,
    {
        let scheduled_run = scheduled.lifecycle_run();

        if trigger == scheduled_run.trigger {
            return self.begin_debounced_async_field_validation(path, id, scheduled);
        }

        let key = ValidatorKey::new(path.identity(), id);
        let target = ValidationTarget::Field(key.field.clone());

        if !scheduled_run.matches_validator(&target, id) {
            return None;
        }

        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_flush_debounced_async_for_trigger(scheduled_run.run_id, trigger)
        {
            return None;
        }

        let run = self.begin_async_field_validation(path, id, trigger)?;
        let outcome = self
            .validation_chains
            .field_validator(&key)
            .expect("validator disappeared during debounced validation flush")
            .lifecycle
            .debounced_async_flushed(trigger);

        self.emit_lifecycle_observer_event(target, outcome);

        Some(run)
    }

    /// Flushes a still-current delayed field-identity validation for a specific trigger.
    pub fn flush_debounced_async_field_identity_validation_for_trigger(
        &mut self,
        field: FieldIdentity,
        id: ValidatorId,
        scheduled: &DebouncedAsyncFieldValidation,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFieldValidation<Model, ()>>
    where
        Model: Clone,
    {
        let scheduled_run = scheduled.lifecycle_run();

        if trigger == scheduled_run.trigger {
            return self.begin_debounced_async_field_identity_validation(field, id, scheduled);
        }

        let key = ValidatorKey::new(field.clone(), id);
        let target = ValidationTarget::Field(key.field.clone());

        if !scheduled_run.matches_validator(&target, id) {
            return None;
        }

        let validator = self.validation_chains.field_validator(&key)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_flush_debounced_async_for_trigger(scheduled_run.run_id, trigger)
        {
            return None;
        }

        let run = self.begin_async_field_identity_validation(field, id, trigger)?;
        let outcome = self
            .validation_chains
            .field_validator(&key)
            .expect("validator disappeared during debounced validation flush")
            .lifecycle
            .debounced_async_flushed(trigger);

        self.emit_lifecycle_observer_event(target, outcome);

        Some(run)
    }

    /// Completes an asynchronous field validation run and applies its source-level result.
    pub fn complete_async_field_validation<Value, Errors>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        run: &AsyncFieldValidation<Model, Value>,
        errors: Errors,
    ) -> Option<ValidationStatus>
    where
        Errors: IntoIterator<Item = Error>,
    {
        let key = ValidatorKey::new(path.identity(), id);

        if run.field != key.field || run.validator_id != id {
            return None;
        }

        let lifecycle_run = run.lifecycle_run();
        let validator = self.validation_chains.field_validator(&key)?;
        let stale_outcome = self.stale_async_completion_outcome(
            &validator.lifecycle,
            &lifecycle_run,
            validator.model_dependent,
        );

        if let Some(outcome) = stale_outcome {
            self.emit_lifecycle_observer_event(lifecycle_run.target.clone(), outcome);
            return None;
        }

        let errors: Vec<_> = errors.into_iter().collect();
        let (status, outcome) = {
            let validator = self
                .validation_chains
                .field_validator_mut(&key)
                .expect("validator disappeared during async validation completion");
            let outcome = validator.lifecycle.complete_async(
                lifecycle_run.trigger,
                run.submit_intent.clone(),
                errors,
            );
            (outcome.status(), outcome)
        };

        self.emit_lifecycle_observer_event(lifecycle_run.target, outcome);

        Some(status)
    }

    /// Completes an asynchronous field-identity validation run and applies its source-level result.
    pub fn complete_async_field_identity_validation<Errors>(
        &mut self,
        field: FieldIdentity,
        id: ValidatorId,
        run: &AsyncFieldValidation<Model, ()>,
        errors: Errors,
    ) -> Option<ValidationStatus>
    where
        Errors: IntoIterator<Item = Error>,
    {
        let key = ValidatorKey::new(field, id);

        if run.field != key.field || run.validator_id != id {
            return None;
        }

        let lifecycle_run = run.lifecycle_run();
        let validator = self.validation_chains.field_validator(&key)?;
        let stale_outcome = self.stale_async_completion_outcome(
            &validator.lifecycle,
            &lifecycle_run,
            validator.model_dependent,
        );

        if let Some(outcome) = stale_outcome {
            self.emit_lifecycle_observer_event(lifecycle_run.target.clone(), outcome);
            return None;
        }

        let errors: Vec<_> = errors.into_iter().collect();
        let (status, outcome) = {
            let validator = self
                .validation_chains
                .field_validator_mut(&key)
                .expect("validator disappeared during async validation completion");
            let outcome = validator.lifecycle.complete_async(
                lifecycle_run.trigger,
                run.submit_intent.clone(),
                errors,
            );
            (outcome.status(), outcome)
        };

        self.emit_lifecycle_observer_event(lifecycle_run.target, outcome);

        Some(status)
    }

    /// Starts an asynchronous form validation run and marks the validator source pending.
    pub fn begin_async_form_validation(
        &mut self,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFormValidation<Model>>
    where
        Model: Clone,
    {
        let validator = self.validation_chains.form_validator(id)?;

        if validator.lifecycle.is_sync() || !validator.lifecycle.should_run(trigger) {
            return None;
        }

        self.with_async_start_sync_gate(ValidationTarget::Form, trigger, |core| {
            core.begin_async_form_validation_after_sync(id, trigger)
        })
    }

    /// Starts an asynchronous form validator after its sync chain has already run.
    pub fn begin_async_form_validation_after_sync(
        &mut self,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFormValidation<Model>>
    where
        Model: Clone,
    {
        let validator = self.validation_chains.form_validator(id)?;

        if !validator.lifecycle.should_begin_async_after_sync(trigger) {
            return None;
        }

        let form = self.draft.current().clone();
        let form_version = self.form_version;
        let submit_intent = self.submit_intent_for_trigger(trigger);
        let (source, run_id, outcome) = {
            let validator = self
                .validation_chains
                .form_validator_mut(id)
                .expect("validator disappeared during async validation scheduling");
            let (run_id, outcome) = validator
                .lifecycle
                .mark_async_started(trigger, submit_intent.clone());
            (outcome.source().clone(), run_id, outcome)
        };

        self.emit_lifecycle_observer_event(ValidationTarget::Form, outcome);

        Some(AsyncFormValidation {
            form: FormSnapshot::new(form),
            validator_id: id,
            source,
            trigger,
            submit_intent,
            form_version,
            run_id,
        })
    }

    /// Marks an asynchronous form validator pending before a delayed value-change run starts.
    pub fn schedule_debounced_async_form_validation(
        &mut self,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<DebouncedAsyncFormValidation> {
        let validator = self.validation_chains.form_validator(id)?;

        if validator.lifecycle.is_sync()
            || !validator.lifecycle.should_schedule_debounced_async(trigger)
        {
            return None;
        }

        self.with_async_start_sync_gate(ValidationTarget::Form, trigger, |core| {
            core.schedule_debounced_async_form_validation_after_sync(id, trigger)
        })
    }

    /// Marks an asynchronous form validator pending after its sync chain has already run.
    pub fn schedule_debounced_async_form_validation_after_sync(
        &mut self,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<DebouncedAsyncFormValidation> {
        let validator = self.validation_chains.form_validator(id)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_schedule_debounced_async_after_sync(trigger)
        {
            return None;
        }

        let submit_intent = self.submit_intent_for_trigger(trigger);
        let (source, run_id, outcome) = {
            let validator = self
                .validation_chains
                .form_validator_mut(id)
                .expect("validator disappeared during debounced validation scheduling");
            let (run_id, outcome) = validator
                .lifecycle
                .mark_debounced_async_pending(trigger, submit_intent);
            (outcome.source().clone(), run_id, outcome)
        };

        self.emit_lifecycle_observer_event(ValidationTarget::Form, outcome);

        Some(DebouncedAsyncFormValidation {
            validator_id: id,
            source,
            trigger,
            run_id,
        })
    }

    /// Starts the latest still-current delayed asynchronous form validation run.
    pub fn begin_debounced_async_form_validation(
        &mut self,
        id: ValidatorId,
        scheduled: &DebouncedAsyncFormValidation,
    ) -> Option<AsyncFormValidation<Model>>
    where
        Model: Clone,
    {
        let scheduled_run = scheduled.lifecycle_run();

        if !scheduled_run.matches_validator(&ValidationTarget::Form, id) {
            return None;
        }

        let validator = self.validation_chains.form_validator(id)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_begin_debounced_async(scheduled_run.trigger, scheduled_run.run_id)
        {
            return None;
        }

        let form = self.draft.current().clone();
        let form_version = self.form_version;
        let (run_id, outcome) = {
            let validator = self
                .validation_chains
                .form_validator_mut(id)
                .expect("validator disappeared during debounced validation start");
            validator
                .lifecycle
                .begin_debounced_async(scheduled_run.trigger)
        };

        self.emit_lifecycle_observer_event(scheduled_run.target.clone(), outcome);

        Some(AsyncFormValidation {
            form: FormSnapshot::new(form),
            validator_id: id,
            source: scheduled.source.clone(),
            trigger: scheduled_run.trigger,
            submit_intent: self.submit_intent_for_trigger(scheduled_run.trigger),
            form_version,
            run_id,
        })
    }

    /// Flushes a still-current delayed form validation for a specific trigger.
    ///
    /// This is used when submit needs to turn a pending value-change debounce into submit-scoped
    /// validation without waiting for the original debounce timer. When the flush changes the
    /// trigger, the target trigger's sync chain runs before async work starts.
    pub fn flush_debounced_async_form_validation_for_trigger(
        &mut self,
        id: ValidatorId,
        scheduled: &DebouncedAsyncFormValidation,
        trigger: ValidationTrigger,
    ) -> Option<AsyncFormValidation<Model>>
    where
        Model: Clone,
    {
        let scheduled_run = scheduled.lifecycle_run();

        if trigger == scheduled_run.trigger {
            return self.begin_debounced_async_form_validation(id, scheduled);
        }

        if !scheduled_run.matches_validator(&ValidationTarget::Form, id) {
            return None;
        }

        let validator = self.validation_chains.form_validator(id)?;

        if validator.lifecycle.is_sync()
            || !validator
                .lifecycle
                .should_flush_debounced_async_for_trigger(scheduled_run.run_id, trigger)
        {
            return None;
        }

        let run = self.begin_async_form_validation(id, trigger)?;
        let outcome = self
            .validation_chains
            .form_validator(id)
            .expect("validator disappeared during debounced form validation flush")
            .lifecycle
            .debounced_async_flushed(trigger);

        self.emit_lifecycle_observer_event(scheduled_run.target.clone(), outcome);

        Some(run)
    }

    /// Completes an asynchronous form validation run and applies its source-level result.
    pub fn complete_async_form_validation<Errors>(
        &mut self,
        id: ValidatorId,
        run: &AsyncFormValidation<Model>,
        errors: Errors,
    ) -> Option<ValidationStatus>
    where
        Errors: IntoIterator<Item = FormValidationError<Error>>,
    {
        if run.validator_id != id {
            return None;
        }

        let lifecycle_run = run.lifecycle_run();
        let validator = self.validation_chains.form_validator(id)?;
        let stale_outcome =
            self.stale_async_completion_outcome(&validator.lifecycle, &lifecycle_run, false);

        if let Some(outcome) = stale_outcome {
            self.emit_lifecycle_observer_event(lifecycle_run.target.clone(), outcome);
            return None;
        }

        let errors: Vec<_> = errors.into_iter().collect();
        let (status, outcome) = {
            let validator = self
                .validation_chains
                .form_validator_mut(id)
                .expect("validator disappeared during async validation completion");
            let outcome = validator.lifecycle.complete_async(
                lifecycle_run.trigger,
                run.submit_intent.clone(),
                errors,
            );
            (outcome.status(), outcome)
        };

        self.emit_lifecycle_observer_event(lifecycle_run.target, outcome);

        Some(status)
    }

    /// Runs all form validators registered for one trigger.
    pub fn validate_form(&mut self, trigger: ValidationTrigger) {
        self.validate_form_chain(trigger);
    }

    /// Runs one form validator by stable ID and trigger.
    ///
    /// Returns `None` when the ID is missing or is not registered for `trigger`.
    pub fn validate_form_validator(
        &mut self,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus> {
        self.validate_form_validator_id(id, trigger)
    }

    /// Runs the first form validator source label registered for the whole form and trigger.
    ///
    /// Returns `None` when the source is missing or is not registered for `trigger`.
    pub fn validate_form_source<Source>(
        &mut self,
        source: Source,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus>
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let id = self.form_validator_id_for_source(&source)?;

        self.validate_form_validator_id(id, trigger)
    }

    /// Runs all validators registered for one trigger.
    pub fn validate_all(&mut self, trigger: ValidationTrigger) {
        self.ensure_all_collection_item_validator_states();
        let fields = self.validation_chains.fields_for_trigger(trigger);

        for field in fields {
            self.validate_field_chain(&field, trigger);
        }

        self.validate_form_chain(trigger);
    }

    /// Explicitly runs validators registered for form initialization.
    ///
    /// Form creation and validator registration never call this automatically.
    /// The returned boolean reflects synchronous initialization validation only. Runtime adapters
    /// may start async initialization validators after calling this method.
    pub fn validate_initialization(&mut self) -> bool {
        self.validate_all(ValidationTrigger::Initial);
        !self.has_validation_errors_for_trigger(ValidationTrigger::Initial)
    }

    /// Records a submit attempt and runs submit-triggered validators.
    pub fn validate_for_submit(&mut self) -> bool {
        self.validate_intent_for_submit(())
    }

    /// Runs submit-triggered synchronous validators for adapter-owned submit preflight.
    ///
    /// Unlike [`Self::validate_for_submit`], this does not mark unresolved async validators as
    /// pending and does not record a submit attempt when the adapter allows submission to continue.
    pub fn validate_for_submit_preflight(&mut self) -> Option<SubmitBlocker> {
        self.validate_intent_for_submit_preflight(())
    }

    fn validate_intent_for_submit<Intent>(&mut self, intent: Intent) -> bool
    where
        Intent: Clone + PartialEq + 'static,
    {
        self.clear_submit_errors();
        self.mark_submit_attempt();
        self.submission
            .set_validation_intent(Some(SubmitIntentSnapshot::new(intent.clone())));
        self.validate_all(ValidationTrigger::Submit);
        self.mark_unresolved_async_validators_pending_for_submit_intent(&intent);
        let valid = !self.has_submit_blocking_errors(&intent)
            && !self.has_pending_validation_for_submit_intent(&intent);

        if !valid {
            self.record_submit_status(
                SubmitStatus::Blocked(self.submit_validation_blocker_for_intent(&intent)),
                intent,
            );
        }

        valid
    }

    fn validate_intent_for_submit_preflight<Intent>(
        &mut self,
        intent: Intent,
    ) -> Option<SubmitBlocker>
    where
        Intent: Clone + PartialEq + 'static,
    {
        if self.submission.is_in_flight() {
            self.record_submit_status(
                SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission),
                intent,
            );
            return Some(SubmitBlocker::InFlightSubmission);
        }

        self.clear_submit_errors();
        self.submission
            .set_validation_intent(Some(SubmitIntentSnapshot::new(intent.clone())));
        self.validate_all(ValidationTrigger::Submit);

        let blocker = if self.has_pending_validation_for_submit_intent(&intent) {
            Some(SubmitBlocker::PendingValidation)
        } else if self.has_submit_blocking_errors(&intent) {
            Some(SubmitBlocker::ValidationErrors)
        } else {
            None
        };

        if let Some(blocker) = blocker {
            self.mark_submit_attempt();
            self.record_submit_status(SubmitStatus::Blocked(blocker), intent);
            Some(blocker)
        } else {
            None
        }
    }

    /// Marks all pending asynchronous validation runs stale so their completions no longer apply.
    pub fn invalidate_pending_async_validations(&mut self) {
        self.invalidate_async_field_validators_for_form_change();
        self.invalidate_pending_async_form_validators();
    }

    /// Records a submit attempt for error visibility decisions and submit-attempt-aware validation modes.
    pub fn mark_submit_attempt(&mut self) {
        let attempt = self.submission.increment_attempt();
        self.emit_observer_event(FormObserverEvent::SubmitAttempted { attempt });
    }

    /// Returns how many submit attempts have been recorded.
    pub const fn submit_attempt_count(&self) -> u64 {
        self.submission.attempt_count()
    }

    /// Marks one registered field validator as skipped and clears its errors.
    pub fn skip_field_validator_by_id<Value>(
        &mut self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
    ) -> bool {
        let key = ValidatorKey::new(path.identity(), id);

        match self.validation_chains.field_validator_mut(&key) {
            Some(validator) => {
                validator.lifecycle.mark_skipped_without_trigger();
                true
            }
            None => false,
        }
    }

    /// Marks the first registered field validator with this source label as skipped and clears its errors.
    pub fn skip_field_validator<Value, Source>(
        &mut self,
        path: FieldPath<Model, Value>,
        source: Source,
    ) -> bool
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let Some(key) = self.field_validator_key_for_source(path.identity(), &source) else {
            return false;
        };

        self.skip_field_validator_by_id(path, key.id)
    }

    /// Marks one registered form validator as skipped and clears its errors.
    pub fn skip_form_validator_by_id(&mut self, id: ValidatorId) -> bool {
        match self.validation_chains.form_validator_mut(id) {
            Some(validator) => {
                validator.lifecycle.mark_skipped_without_trigger();
                true
            }
            None => false,
        }
    }

    /// Marks the first registered form validator with this source label as skipped and clears its errors.
    pub fn skip_form_validator<Source>(&mut self, source: Source) -> bool
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let Some(id) = self.form_validator_id_for_source(&source) else {
            return false;
        };

        self.skip_form_validator_by_id(id)
    }

    /// Returns the current status for one registered field validator.
    pub fn field_validation_status<Value>(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
    ) -> Option<ValidationStatus> {
        self.validation_chains
            .field_validator(&ValidatorKey::new(path.identity(), id))
            .map(|validator| validator.lifecycle.status())
    }

    /// Returns the current status for the first registered field validator with this source label.
    pub fn validation_status<Value, Source>(
        &self,
        path: FieldPath<Model, Value>,
        source: Source,
    ) -> Option<ValidationStatus>
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let key = self.field_validator_key_for_source(path.identity(), &source)?;

        self.field_validation_status(path, key.id)
    }

    /// Returns the current status for one registered form validator.
    pub fn form_validation_status_by_id(&self, id: ValidatorId) -> Option<ValidationStatus> {
        self.validation_chains
            .form_validator(id)
            .map(|validator| validator.lifecycle.status())
    }

    /// Returns the current status for the first registered form validator with this source label.
    pub fn form_validation_status<Source>(&self, source: Source) -> Option<ValidationStatus>
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let id = self.form_validator_id_for_source(&source)?;

        self.form_validation_status_by_id(id)
    }

    /// Returns source-level validation statuses in deterministic flattened order.
    ///
    /// Field-validator statuses are listed before form-validator statuses. Each category follows
    /// stable validator registration order.
    pub fn validation_statuses(&self) -> Vec<ValidationStatusView> {
        self.validation_chains.validation_statuses()
    }

    /// Returns source-level validation statuses for one field in stable registration order.
    pub fn field_validation_statuses<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> Vec<ValidationStatusView> {
        let field = path.identity();

        self.validation_chains.field_validation_statuses(&field)
    }

    /// Returns source-level validation statuses for one field identity in stable registration order.
    pub fn field_validation_statuses_by_identity(
        &self,
        field: &FieldIdentity,
    ) -> Vec<ValidationStatusView> {
        self.validation_chains
            .field_identity_validation_statuses(field)
    }

    /// Returns source-level validation statuses for form validators in stable registration order.
    pub fn form_validation_statuses(&self) -> Vec<ValidationStatusView> {
        self.validation_chains.form_validation_statuses()
    }

    /// Returns validation errors in deterministic flattened order.
    ///
    /// Field-validator errors are listed before form-validator errors, followed by submit errors.
    /// Validator categories follow stable registration order and each source preserves its own
    /// error order.
    pub fn validation_errors(&self) -> Vec<ValidationErrorView<'_, Error>> {
        self.validation_errors_matching(|_| true)
    }

    /// Returns validation errors for one field in deterministic flattened order.
    pub fn field_validation_errors<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> Vec<ValidationErrorView<'_, Error>> {
        let field = path.identity();

        self.validation_errors_matching(|target| target.as_field() == Some(&field))
    }

    /// Returns validation errors for one field identity in deterministic flattened order.
    pub fn field_validation_errors_by_identity(
        &self,
        field: &FieldIdentity,
    ) -> Vec<ValidationErrorView<'_, Error>> {
        self.validation_errors_matching(|target| target.as_field() == Some(field))
    }

    /// Returns form-level validation errors in deterministic flattened order.
    pub fn form_validation_errors(&self) -> Vec<ValidationErrorView<'_, Error>> {
        self.validation_errors_matching(ValidationTarget::is_form)
    }

    /// Returns validation errors visible under the default blur-or-submit policy.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorView<'_, Error>> {
        self.validation_errors_matching(|target| self.should_show_validation_errors(target))
    }

    /// Returns visible validation errors relevant to one submit intent.
    pub fn visible_validation_errors_for_intent<Intent>(
        &self,
        intent: &Intent,
    ) -> Vec<ValidationErrorView<'_, Error>>
    where
        Intent: PartialEq + 'static,
    {
        self.validation_errors_matching_for_submit_intent(intent, |target| {
            self.should_show_validation_errors(target)
        })
    }

    /// Returns visible validation errors for one field.
    pub fn visible_field_validation_errors<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> Vec<ValidationErrorView<'_, Error>> {
        let field = path.identity();

        self.validation_errors_matching(|target| {
            target.as_field() == Some(&field) && self.should_show_validation_errors(target)
        })
    }

    /// Returns visible validation errors relevant to one submit intent for one field.
    pub fn visible_field_validation_errors_for_intent<Value, Intent>(
        &self,
        path: FieldPath<Model, Value>,
        intent: &Intent,
    ) -> Vec<ValidationErrorView<'_, Error>>
    where
        Intent: PartialEq + 'static,
    {
        let field = path.identity();

        self.validation_errors_matching_for_submit_intent(intent, |target| {
            target.as_field() == Some(&field) && self.should_show_validation_errors(target)
        })
    }

    /// Returns visible validation errors for one field identity.
    pub fn visible_field_validation_errors_by_identity(
        &self,
        field: &FieldIdentity,
    ) -> Vec<ValidationErrorView<'_, Error>> {
        self.validation_errors_matching(|target| {
            target.as_field() == Some(field) && self.should_show_validation_errors(target)
        })
    }

    /// Returns visible validation errors relevant to one submit intent for one field identity.
    pub fn visible_field_validation_errors_by_identity_for_intent<Intent>(
        &self,
        field: &FieldIdentity,
        intent: &Intent,
    ) -> Vec<ValidationErrorView<'_, Error>>
    where
        Intent: PartialEq + 'static,
    {
        self.validation_errors_matching_for_submit_intent(intent, |target| {
            target.as_field() == Some(field) && self.should_show_validation_errors(target)
        })
    }

    /// Returns visible form-level validation errors.
    pub fn visible_form_validation_errors(&self) -> Vec<ValidationErrorView<'_, Error>> {
        self.validation_errors_matching(|target| {
            target.is_form() && self.should_show_validation_errors(target)
        })
    }

    /// Returns visible form-level validation errors relevant to one submit intent.
    pub fn visible_form_validation_errors_for_intent<Intent>(
        &self,
        intent: &Intent,
    ) -> Vec<ValidationErrorView<'_, Error>>
    where
        Intent: PartialEq + 'static,
    {
        self.validation_errors_matching_for_submit_intent(intent, |target| {
            target.is_form() && self.should_show_validation_errors(target)
        })
    }

    fn replace_field_with_origin<Value>(
        &mut self,
        path: &FieldPath<Model, Value>,
        value: Value,
        origin: FieldUpdateOrigin,
    ) {
        let field = FormObserverField::from_path(path);
        let field_identity = field.identity();

        *path.get_mut(self.draft.current_mut()) = value;
        self.increment_form_version();
        self.increment_field_version(&field_identity);
        self.invalidate_async_field_validators_for_model_change();
        self.invalidate_pending_async_form_validators();
        self.clear_submit_errors_for_field(&field_identity);
        self.emit_observer_event(FormObserverEvent::FieldUpdated {
            field,
            origin,
            value: FormObserverValue::Redacted,
        });
    }

    /// Replaces a typed field value in the current draft.
    pub fn set_field<Value>(&mut self, path: FieldPath<Model, Value>, value: Value) {
        self.replace_field_with_origin(&path, value, FieldUpdateOrigin::Programmatic);
        self.validate_value_change_if_configured(path);
    }

    /// Replaces one typed field value because of user input.
    pub fn set_user_field<Value>(&mut self, path: FieldPath<Model, Value>, value: Value) {
        self.replace_field_with_origin(&path, value, FieldUpdateOrigin::User);
        self.mark_field_touched(path.clone());
        self.validate_value_change_if_configured(path);
    }

    /// Marks a field as touched by user interaction.
    pub fn mark_field_touched<Value>(&mut self, path: FieldPath<Model, Value>) {
        self.field_metadata_mut(&path.identity()).touched = true;
    }

    /// Marks a field identity as touched by user interaction.
    pub fn mark_field_identity_touched(&mut self, field: &FieldIdentity) {
        self.field_metadata_mut(field).touched = true;
    }

    /// Records a user change for field-like state that lives outside the form draft.
    ///
    /// This preserves the same field-scoped lifecycle invariants as ordinary field replacement
    /// without mutating the form model: stale submit errors are cleared and submit snapshots use a
    /// new field version. The form version stays scoped to draft/model edits so pending ordinary
    /// async field validators do not become stale because adapter-owned field-like state changed.
    pub fn record_field_identity_user_change(&mut self, field: &FieldIdentity) {
        self.increment_field_version(field);
        self.invalidate_async_field_validators_for_field(field);
        self.invalidate_pending_async_form_validators();
        self.clear_submit_errors_for_field(field);
        self.field_metadata_mut(field).touched = true;

        if self
            .validation_mode
            .should_validate_on_change(self.submission.attempt_count())
        {
            self.validate_field_chain(field, ValidationTrigger::Change);
            self.validate_form_chain(ValidationTrigger::Change);
        }
    }

    /// Marks a field identity as blurred and touched by user interaction.
    pub fn mark_field_identity_blurred(&mut self, field: &FieldIdentity) {
        let metadata = self.field_metadata_mut(field);
        metadata.touched = true;
        metadata.blurred = true;

        if self
            .validation_mode
            .should_validate_on_blur(self.submission.attempt_count())
        {
            self.validate_field_chain(field, ValidationTrigger::Blur);
            self.validate_form_chain(ValidationTrigger::Blur);
        }
    }

    /// Marks a field as blurred and touched by user interaction.
    pub fn mark_field_blurred<Value>(&mut self, path: FieldPath<Model, Value>) {
        self.mark_field_blurred_without_validation(path.clone());

        if self
            .validation_mode
            .should_validate_on_blur(self.submission.attempt_count())
        {
            self.validate_field(path, ValidationTrigger::Blur);
        }
    }

    /// Marks a field as blurred and touched without running blur validation.
    pub fn mark_field_blurred_without_validation<Value>(&mut self, path: FieldPath<Model, Value>) {
        let metadata = self.field_metadata_mut(&path.identity());
        metadata.touched = true;
        metadata.blurred = true;
    }

    fn ensure_collection_state<Item>(
        &mut self,
        path: &FieldPath<Model, Vec<Item>>,
    ) -> &mut CollectionState {
        let identity = path.identity();
        let baseline_len = path.get(self.draft.baseline()).len();
        let current_len = path.get(self.draft.current()).len();

        self.field_store
            .collection_or_insert_with(identity, || CollectionState::new(baseline_len, current_len))
    }

    fn ensure_collection_item_validator_states_for_collection(
        &mut self,
        collection: &FieldIdentity,
    ) {
        let Some(collection_path) = collection.as_static_path() else {
            return;
        };
        let Some(items) = self
            .field_store
            .collection(collection)
            .map(|state| state.current_items.clone())
        else {
            return;
        };
        let templates: Vec<_> = self
            .validation_chains
            .collection_item_template_entries()
            .filter(|(key, _)| key.collection == *collection)
            .map(|(key, validator)| {
                (
                    key.id,
                    key.field.clone(),
                    validator.source.clone(),
                    validator.triggers.clone(),
                )
            })
            .collect();

        for item in items {
            for (id, field, source, triggers) in &templates {
                let field = CollectionItemFieldAddress::identity_from_static_segments(
                    collection_path,
                    item,
                    field.clone(),
                );
                let key = ValidatorKey::new(field, *id);

                self.validation_chains.ensure_collection_item_state(
                    key,
                    source.clone(),
                    triggers.clone(),
                );
            }
        }
    }

    fn ensure_all_collection_item_validator_states(&mut self) {
        let template_collections: Vec<_> = self
            .validation_chains
            .collection_item_template_entries()
            .map(|(key, validator)| {
                (
                    key.collection.clone(),
                    (validator.collection_len)(self.draft.baseline()),
                    (validator.collection_len)(self.draft.current()),
                )
            })
            .collect();

        for (collection, baseline_len, current_len) in template_collections {
            self.field_store.collection_or_insert_with(collection, || {
                CollectionState::new(baseline_len, current_len)
            });
        }

        let collections: Vec<_> = self.field_store.collection_keys();

        for collection in collections {
            self.ensure_collection_item_validator_states_for_collection(&collection);
        }
    }

    fn collection_item_exists<Item>(
        &self,
        path: &FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
    ) -> bool {
        self.field_store
            .collection(&path.identity())
            .is_some_and(|state| state.current_index(item).is_some())
    }

    fn insert_collection_item_with_origin<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
        origin: FieldUpdateOrigin,
    ) -> Option<CollectionItemIdentity> {
        let current_len = path.get(self.draft.current()).len();
        if index > current_len {
            return None;
        }

        let collection = path.identity();
        let item_identity = self.ensure_collection_state(&path).allocate_item_identity();
        path.get_mut(self.draft.current_mut()).insert(index, item);
        self.field_store
            .collection_mut(&collection)
            .expect("collection state should exist after identity allocation")
            .current_items
            .insert(index, item_identity);
        self.ensure_collection_item_validator_states_for_collection(&collection);
        self.after_collection_mutation(&collection, origin);
        self.validate_inserted_collection_item_if_configured(&collection, item_identity);
        self.emit_observer_event(FormObserverEvent::CollectionItemInserted {
            collection,
            item: item_identity,
            index,
            origin,
            value: FormObserverValue::Redacted,
        });

        Some(item_identity)
    }

    fn remove_collection_item_with_origin<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        origin: FieldUpdateOrigin,
    ) -> Option<Item> {
        let collection = path.identity();
        let index = self.ensure_collection_state(&path).current_index(item)?;
        let removed = path.get_mut(self.draft.current_mut()).remove(index);
        self.field_store
            .collection_mut(&collection)
            .expect("collection state should exist before item removal")
            .current_items
            .remove(index);
        self.clear_collection_item_state(&collection, item);
        self.after_collection_mutation(&collection, origin);
        self.emit_observer_event(FormObserverEvent::CollectionItemRemoved {
            collection,
            item,
            index,
            origin,
            value: FormObserverValue::Redacted,
        });

        Some(removed)
    }

    fn move_collection_item_to_index_with_origin<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        index: usize,
        origin: FieldUpdateOrigin,
    ) -> bool {
        let collection = path.identity();
        let len = path.get(self.draft.current()).len();
        if index >= len {
            return false;
        }

        let from = match self.ensure_collection_state(&path).current_index(item) {
            Some(from) => from,
            None => return false,
        };
        if from == index {
            return true;
        }

        let value = path.get_mut(self.draft.current_mut()).remove(from);
        path.get_mut(self.draft.current_mut()).insert(index, value);
        let state = self
            .field_store
            .collection_mut(&collection)
            .expect("collection state should exist before item move");
        let moved = state.current_items.remove(from);
        state.current_items.insert(index, moved);
        self.after_collection_mutation(&collection, origin);
        self.emit_observer_event(FormObserverEvent::CollectionItemMoved {
            collection,
            item,
            from,
            to: index,
            origin,
        });

        true
    }

    fn swap_collection_items_with_origin<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        a: usize,
        b: usize,
        origin: FieldUpdateOrigin,
    ) -> bool {
        let collection = path.identity();
        let len = path.get(self.draft.current()).len();
        if a >= len || b >= len {
            return false;
        }
        if a == b {
            return false;
        }

        self.ensure_collection_state(&path);
        path.get_mut(self.draft.current_mut()).swap(a, b);
        let state = self
            .field_store
            .collection_mut(&collection)
            .expect("collection state should exist before item swap");
        let first = state.current_items[a];
        let second = state.current_items[b];
        state.current_items.swap(a, b);
        self.after_collection_mutation(&collection, origin);
        self.emit_observer_event(FormObserverEvent::CollectionItemsSwapped {
            collection,
            first,
            second,
            origin,
        });

        true
    }

    fn replace_collection_item_with_origin<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
        origin: FieldUpdateOrigin,
    ) -> bool {
        let collection = path.identity();
        {
            let items = path.get_mut(self.draft.current_mut());
            if index >= items.len() {
                return false;
            }
            items[index] = item;
        }

        self.ensure_collection_state(&path);
        let item_identity = self
            .field_store
            .collection(&collection)
            .and_then(|state| state.current_items.get(index).copied())
            .expect("collection item identity should exist at a valid index");
        // The whole item value changed, so submit errors previously attached to this item's child
        // fields no longer describe the current value. Clear them, matching the per-child replace
        // path (`replace_collection_item_field_with_origin`) and item removal. Sync validation
        // results recompute through the configured value-change trigger, exactly like `set_field`.
        self.submission.retain_errors(|error| {
            !error.target.as_field().is_some_and(|field| {
                CollectionItemFieldAddress::matches_item(field, &collection, item_identity)
            })
        });
        self.after_collection_mutation(&collection, origin);
        self.emit_observer_event(FormObserverEvent::CollectionItemReplaced {
            collection,
            item: item_identity,
            index,
            origin,
            value: FormObserverValue::Redacted,
        });

        true
    }

    fn clear_collection_items_with_origin<Item>(
        &mut self,
        path: FieldPath<Model, Vec<Item>>,
        origin: FieldUpdateOrigin,
    ) -> Vec<CollectionItemIdentity> {
        let collection = path.identity();
        self.ensure_collection_state(&path);
        let cleared: Vec<CollectionItemIdentity> = self
            .field_store
            .collection(&collection)
            .map(|state| state.current_items.clone())
            .unwrap_or_default();

        if cleared.is_empty() {
            return cleared;
        }

        path.get_mut(self.draft.current_mut()).clear();
        if let Some(state) = self.field_store.collection_mut(&collection) {
            state.current_items.clear();
        }
        for item in &cleared {
            self.clear_collection_item_state(&collection, *item);
        }
        self.after_collection_mutation(&collection, origin);
        self.emit_observer_event(FormObserverEvent::CollectionCleared { collection, origin });

        cleared
    }

    fn replace_collection_item_field_with_origin<Item, Value>(
        &mut self,
        collection: &FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: &FieldPath<Item, Value>,
        value: Value,
        origin: FieldUpdateOrigin,
    ) -> bool {
        let Some(index) = self.ensure_collection_state(collection).current_index(item) else {
            return false;
        };
        let address = CollectionItemFieldAddress::new(collection, item, index, field);
        let identity = address.identity();
        let field_name = address.field_name().to_owned();
        let Some(item_value) = collection.get_mut(self.draft.current_mut()).get_mut(index) else {
            return false;
        };

        *field.get_mut(item_value) = value;
        let collection_identity = collection.identity();
        self.increment_form_version();
        self.increment_field_version(&collection_identity);
        self.increment_field_version(&identity);
        self.invalidate_async_field_validators_for_model_change();
        self.invalidate_pending_async_form_validators();
        self.clear_submit_errors_for_field(&collection_identity);
        self.clear_submit_errors_for_field(&identity);
        self.emit_observer_event(FormObserverEvent::FieldUpdated {
            field: FormObserverField::new(identity.clone(), field_name),
            origin,
            value: FormObserverValue::Redacted,
        });
        self.validate_collection_item_field_value_change_if_configured(
            &collection_identity,
            &identity,
        );

        true
    }

    fn after_collection_mutation(&mut self, collection: &FieldIdentity, origin: FieldUpdateOrigin) {
        self.increment_form_version();
        self.increment_field_version(collection);
        self.invalidate_async_field_validators_for_model_change();
        self.invalidate_pending_async_form_validators();
        self.clear_submit_errors_for_field(collection);

        if matches!(origin, FieldUpdateOrigin::User) {
            self.field_metadata_mut(collection).touched = true;
        }

        self.validate_collection_value_change_if_configured_by_identity(collection);
    }

    fn validate_collection_value_change_if_configured_by_identity(
        &mut self,
        collection: &FieldIdentity,
    ) {
        if self
            .validation_mode
            .should_validate_on_change(self.submission.attempt_count())
        {
            self.validate_field_chain(collection, ValidationTrigger::Change);
            self.validate_form_chain(ValidationTrigger::Change);
        }
    }

    fn validate_collection_item_field_value_change_if_configured(
        &mut self,
        collection: &FieldIdentity,
        field: &FieldIdentity,
    ) {
        if self
            .validation_mode
            .should_validate_on_change(self.submission.attempt_count())
        {
            self.validate_field_chain(field, ValidationTrigger::Change);
            self.validate_field_chain(collection, ValidationTrigger::Change);
            self.validate_form_chain(ValidationTrigger::Change);
        }
    }

    fn validate_inserted_collection_item_if_configured(
        &mut self,
        collection: &FieldIdentity,
        item: CollectionItemIdentity,
    ) {
        if !self
            .validation_mode
            .should_validate_on_change(self.submission.attempt_count())
        {
            return;
        }
        let Some(collection_path) = collection.as_static_path() else {
            return;
        };
        let fields: Vec<_> = self
            .validation_chains
            .collection_item_template_keys()
            .filter(|key| key.collection == *collection)
            .map(|key| {
                CollectionItemFieldAddress::identity_from_static_segments(
                    collection_path,
                    item,
                    key.field.clone(),
                )
            })
            .collect();

        for field in fields {
            self.validate_field_chain(&field, ValidationTrigger::Change);
        }
    }

    fn validate_collection_item_field_blur<Item, Value>(
        &mut self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) {
        let field = collection_item_field_identity(&collection, item, &field);
        self.validate_field_chain(&field, ValidationTrigger::Blur);
        self.validate_form_chain(ValidationTrigger::Blur);
    }

    fn clear_collection_item_state(
        &mut self,
        collection: &FieldIdentity,
        item: CollectionItemIdentity,
    ) {
        self.field_store.retain_fields(|field| {
            !CollectionItemFieldAddress::matches_item(field, collection, item)
        });
        self.validation_chains.retain_field_validators(|key| {
            !CollectionItemFieldAddress::matches_item(&key.field, collection, item)
        });
        self.validation_chains.retain_collection_item_states(|key| {
            !CollectionItemFieldAddress::matches_item(&key.field, collection, item)
        });
        self.validation_chains.retain_form_errors(|error| {
            !error.target.as_field().is_some_and(|field| {
                CollectionItemFieldAddress::matches_item(field, collection, item)
            })
        });
        self.submission.retain_errors(|error| {
            !error.target.as_field().is_some_and(|field| {
                CollectionItemFieldAddress::matches_item(field, collection, item)
            })
        });
    }

    fn field_metadata_mut(&mut self, identity: &FieldIdentity) -> &mut FieldMetadata {
        self.field_store.metadata_mut(identity)
    }

    fn increment_field_version(&mut self, identity: &FieldIdentity) {
        self.field_store.increment_version(identity);
    }

    fn current_field_version(&self, identity: &FieldIdentity) -> u64 {
        self.field_store.version(identity)
    }

    fn submit_intent_ref_for_trigger(&self, trigger: ValidationTrigger) -> Option<&dyn Any> {
        if trigger == ValidationTrigger::Submit {
            self.submission
                .validation_intent()
                .map(SubmitIntentSnapshot::as_any)
        } else {
            None
        }
    }

    fn submit_intent_for_trigger(
        &self,
        trigger: ValidationTrigger,
    ) -> Option<SubmitIntentSnapshot> {
        if trigger == ValidationTrigger::Submit {
            self.submission.validation_intent().cloned()
        } else {
            None
        }
    }

    fn increment_form_version(&mut self) {
        self.form_version = self
            .form_version
            .checked_add(1)
            .expect("form version counter exhausted");
    }

    fn record_submit_status<Intent>(&mut self, status: SubmitStatus, intent: Intent)
    where
        Intent: 'static,
    {
        self.submission
            .record_status(StoredLastSubmitStatus::new(status, intent));
    }

    fn record_submit_status_snapshot(
        &mut self,
        status: SubmitStatus,
        intent: SubmitIntentSnapshot,
    ) {
        self.submission
            .record_status(StoredLastSubmitStatus::with_snapshot(status, intent));
    }

    fn take_submission_in_flight_intent(&mut self) -> SubmitIntentSnapshot {
        self.submission
            .take_in_flight_intent()
            .unwrap_or_else(|| SubmitIntentSnapshot::new(()))
    }

    fn async_run_is_stale(&self, run: &AsyncValidationRun, model_dependent: bool) -> bool {
        match &run.target {
            ValidationTarget::Field(field) if field.is_file() && !model_dependent => {
                self.current_field_version(field)
                    != run
                        .field_version
                        .expect("field async validation run has no field version")
            }
            ValidationTarget::Field(field) => validation_lifecycle::is_async_field_stale(
                self.form_version,
                run.form_version,
                self.current_field_version(field),
                run.field_version
                    .expect("field async validation run has no field version"),
            ),
            ValidationTarget::Form => {
                validation_lifecycle::is_async_form_stale(self.form_version, run.form_version)
            }
        }
    }

    fn stale_async_completion_outcome<StoredError>(
        &self,
        lifecycle: &validation_lifecycle::SourceState<StoredError>,
        run: &AsyncValidationRun,
        model_dependent: bool,
    ) -> Option<validation_lifecycle::TransitionOutcome> {
        if lifecycle.should_ignore_async_completion(run.run_id)
            || self.async_run_is_stale(run, model_dependent)
        {
            Some(lifecycle.stale_async_completion_ignored(run.trigger))
        } else {
            None
        }
    }

    fn invalidate_async_field_validators_for_form_change(&mut self) {
        self.invalidate_async_field_validators(|_, _| true);
    }

    fn invalidate_async_field_validators_for_model_change(&mut self) {
        // Async field validators receive the whole form snapshot, so any draft edit can make
        // their pending or completed result describe stale context. File-selection validators that
        // do not request form context are scoped to adapter-owned file state and should not be
        // invalidated by ordinary draft edits.
        self.invalidate_async_field_validators(|key, validator| {
            !key.field.is_file() || validator.model_dependent
        });
    }

    fn invalidate_async_field_validators_for_field(&mut self, field: &FieldIdentity) {
        self.invalidate_async_field_validators(|key, _| key.field == *field);
    }

    fn invalidate_async_field_validators(
        &mut self,
        mut include: impl FnMut(&ValidatorKey, &RegisteredFieldValidator<Model, Error>) -> bool,
    ) {
        let keys: Vec<_> = self
            .validation_chains
            .sorted_field_entries()
            .into_iter()
            .filter(|(key, validator)| include(key, validator))
            .map(|(key, _)| key.clone())
            .collect();

        for key in keys {
            let Some(validator) = self.validation_chains.field_validator_mut(&key) else {
                continue;
            };

            if validator.lifecycle.is_sync()
                || !matches!(
                    validator.lifecycle.status(),
                    ValidationStatus::Pending | ValidationStatus::Valid | ValidationStatus::Invalid
                )
            {
                continue;
            }

            validator.lifecycle.mark_stale();
        }
    }

    fn invalidate_pending_async_form_validators(&mut self) {
        for validator in self.validation_chains.form_values_mut() {
            if validator.lifecycle.is_sync()
                || !matches!(
                    validator.lifecycle.status(),
                    ValidationStatus::Pending | ValidationStatus::Valid | ValidationStatus::Invalid
                )
            {
                continue;
            }

            validator.lifecycle.mark_stale();
        }
    }

    fn validate_value_change_if_configured<Value>(&mut self, path: FieldPath<Model, Value>) {
        if self
            .validation_mode
            .should_validate_on_change(self.submission.attempt_count())
        {
            self.validate_field(path, ValidationTrigger::Change);
        }
    }

    fn emit_observer_event(&mut self, event: FormObserverEvent) {
        for observer in &mut self.observers {
            observer(&event);
        }
    }

    fn emit_lifecycle_observer_event(
        &mut self,
        target: ValidationTarget,
        outcome: validation_lifecycle::TransitionOutcome,
    ) {
        self.emit_observer_event(Self::observer_event_for_lifecycle_outcome(target, outcome));
    }

    fn observer_event_for_lifecycle_outcome(
        target: ValidationTarget,
        outcome: validation_lifecycle::TransitionOutcome,
    ) -> FormObserverEvent {
        let source = outcome.source().clone();
        let trigger = outcome.trigger();
        let status = outcome.status();

        match outcome.kind() {
            validation_lifecycle::TransitionKind::ValidationRan => {
                FormObserverEvent::ValidationRan {
                    target,
                    source,
                    trigger,
                    status,
                }
            }
            validation_lifecycle::TransitionKind::AsyncValidationScheduled => {
                FormObserverEvent::AsyncValidationScheduled {
                    target,
                    source,
                    trigger,
                    status,
                }
            }
            validation_lifecycle::TransitionKind::AsyncValidationCompleted => {
                FormObserverEvent::AsyncValidationCompleted {
                    target,
                    source,
                    trigger,
                    status,
                }
            }
            validation_lifecycle::TransitionKind::AsyncValidationSkipped => {
                FormObserverEvent::AsyncValidationSkipped {
                    target,
                    source,
                    trigger,
                    status,
                }
            }
            validation_lifecycle::TransitionKind::AsyncValidationStaleIgnored => {
                FormObserverEvent::AsyncValidationStaleIgnored {
                    target,
                    source,
                    trigger,
                    status,
                }
            }
            validation_lifecycle::TransitionKind::DebouncedAsyncValidationScheduled => {
                FormObserverEvent::DebouncedAsyncValidationScheduled {
                    target,
                    source,
                    trigger,
                    status,
                }
            }
            validation_lifecycle::TransitionKind::DebouncedAsyncValidationFlushed => {
                FormObserverEvent::DebouncedAsyncValidationFlushed {
                    target,
                    source,
                    trigger,
                    status,
                }
            }
        }
    }

    fn clear_validation_results(&mut self) {
        self.submission.reset();
        self.validation_chains.clear_results();
    }

    fn has_validation_errors(&self) -> bool {
        self.validation_chains
            .field_values()
            .any(|validator| !validator.lifecycle.errors().is_empty())
            || self
                .validation_chains
                .collection_item_state_values()
                .any(|validator| !validator.errors().is_empty())
            || self
                .validation_chains
                .form_values()
                .any(|validator| !validator.lifecycle.errors().is_empty())
            || self.submission.has_errors()
    }

    fn has_submit_blocking_errors<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.validation_chains.has_errors_blocking_submit(intent)
            || self.submission_errors_block_submit_intent(intent)
    }

    fn has_known_submit_blocking_errors<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.validation_chains
            .has_known_errors_affecting_availability(intent)
            || self.submission_errors_block_submit_intent(intent)
    }

    fn submission_errors_block_submit_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.submission
            .errors()
            .iter()
            .any(|error| self.submit_error_blocks_submit_intent(error, intent))
    }

    fn submit_error_blocks_submit_intent<Intent>(
        &self,
        error: &StoredSubmitError<Error>,
        intent: &Intent,
    ) -> bool
    where
        Intent: PartialEq + 'static,
    {
        error
            .submit_intent
            .as_ref()
            .is_none_or(|submit_intent| submit_intent.matches(intent))
    }

    fn has_validation_errors_for_trigger(&self, trigger: ValidationTrigger) -> bool {
        self.validation_chains.field_values().any(|validator| {
            validator.lifecycle.should_run(trigger) && !validator.lifecycle.errors().is_empty()
        }) || self
            .validation_chains
            .collection_item_state_values()
            .any(|validator| validator.should_run(trigger) && !validator.errors().is_empty())
            || self.validation_chains.form_values().any(|validator| {
                validator.lifecycle.should_run(trigger) && !validator.lifecycle.errors().is_empty()
            })
    }

    fn has_pending_validation_for_submit_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.validation_chains.has_pending_submit_validation(intent)
    }

    fn has_pending_validation_for_trigger(&self, trigger: ValidationTrigger) -> bool {
        self.validation_chains.field_values().any(|validator| {
            validator.lifecycle.should_run(trigger)
                && validator.lifecycle.status() == ValidationStatus::Pending
        }) || self
            .validation_chains
            .collection_item_state_values()
            .any(|validator| {
                validator.should_run(trigger) && validator.status() == ValidationStatus::Pending
            })
            || self.validation_chains.form_values().any(|validator| {
                validator.lifecycle.should_run(trigger)
                    && validator.lifecycle.status() == ValidationStatus::Pending
            })
    }

    fn has_unresolved_async_validation_for_submit_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.validation_chains.has_unresolved_submit_async(intent)
    }

    fn mark_unresolved_async_validators_pending_for_submit_intent<Intent>(
        &mut self,
        intent: &Intent,
    ) where
        Intent: PartialEq + 'static,
    {
        let mut events = Vec::new();
        let field_keys = self
            .validation_chains
            .unresolved_submit_field_async_keys(intent);

        let submit_intent = self.submit_intent_for_trigger(ValidationTrigger::Submit);

        for key in field_keys {
            let Some(validator) = self.validation_chains.field_validator_mut(&key) else {
                continue;
            };

            let (_, outcome) = validator
                .lifecycle
                .mark_async_required(ValidationTrigger::Submit, submit_intent.clone());
            events.push((ValidationTarget::Field(key.field), outcome));
        }

        let form_ids = self
            .validation_chains
            .unresolved_submit_form_async_ids(intent);

        let submit_intent = self.submit_intent_for_trigger(ValidationTrigger::Submit);

        for id in form_ids {
            let Some(validator) = self.validation_chains.form_validator_mut(id) else {
                continue;
            };

            let (_, outcome) = validator
                .lifecycle
                .mark_async_required(ValidationTrigger::Submit, submit_intent.clone());
            events.push((ValidationTarget::Form, outcome));
        }

        for (target, outcome) in events {
            self.emit_lifecycle_observer_event(target, outcome);
        }
    }

    fn allocate_validator_id(&mut self) -> ValidatorId {
        self.validation_chains.allocate_validator_id()
    }

    fn field_validator_key_for_source(
        &self,
        field: FieldIdentity,
        source: &ValidatorSource,
    ) -> Option<ValidatorKey> {
        self.validation_chains
            .field_validator_key_for_source(field, source)
    }

    fn form_validator_id_for_source(&self, source: &ValidatorSource) -> Option<ValidatorId> {
        self.validation_chains.form_validator_id_for_source(source)
    }

    fn with_async_start_sync_gate<Run>(
        &mut self,
        target: ValidationTarget,
        trigger: ValidationTrigger,
        start: impl FnOnce(&mut Self) -> Option<Run>,
    ) -> Option<Run> {
        match target {
            ValidationTarget::Field(field) => {
                if !self.validate_field_sync_chain(&field, trigger) {
                    self.skip_async_field_validators_for_chain(&field, trigger);
                    self.validate_form_chain(trigger);
                    return None;
                }

                self.clear_skipped_async_field_validators_for_chain(&field, trigger);
                let run = start(self);
                self.validate_form_chain(trigger);
                run
            }
            ValidationTarget::Form => {
                if !self.validate_form_sync_chain(trigger) {
                    self.skip_async_form_validators_for_chain(trigger);
                    return None;
                }

                self.clear_skipped_async_form_validators_for_chain(trigger);
                start(self)
            }
        }
    }

    fn validate_field_chain(&mut self, field: &FieldIdentity, trigger: ValidationTrigger) {
        if !self.validate_field_sync_chain(field, trigger) {
            self.skip_async_field_validators_for_chain(field, trigger);
        } else {
            self.clear_skipped_async_field_validators_for_chain(field, trigger);
        }
    }

    fn validate_field_sync_chain(
        &mut self,
        field: &FieldIdentity,
        trigger: ValidationTrigger,
    ) -> bool {
        self.ensure_all_collection_item_validator_states();
        let keys = self
            .validation_chains
            .sync_field_keys_for_chain(field, trigger);
        let collection_item_keys = self
            .validation_chains
            .sync_collection_item_keys_for_chain(field, trigger);
        let mut valid = true;

        for key in keys {
            if self.validate_field_validator_key(key, trigger) == Some(ValidationStatus::Invalid) {
                valid = false;
            }
        }

        for key in collection_item_keys {
            if self.validate_collection_item_field_validator_key(key, trigger)
                == Some(ValidationStatus::Invalid)
            {
                valid = false;
            }
        }

        valid
    }

    fn skip_async_field_validators_for_chain(
        &mut self,
        field: &FieldIdentity,
        trigger: ValidationTrigger,
    ) {
        let mut events = Vec::new();
        let keys = self
            .validation_chains
            .async_field_keys_to_skip(field, trigger);

        let submit_intent = self.submit_intent_for_trigger(trigger);

        for key in keys {
            let Some(validator) = self.validation_chains.field_validator_mut(&key) else {
                continue;
            };

            let outcome = validator
                .lifecycle
                .skip_async(trigger, submit_intent.clone());
            events.push((ValidationTarget::Field(key.field.clone()), outcome));
        }

        for (target, outcome) in events {
            self.emit_lifecycle_observer_event(target, outcome);
        }
    }

    fn clear_skipped_async_field_validators_for_chain(
        &mut self,
        field: &FieldIdentity,
        trigger: ValidationTrigger,
    ) {
        let keys = self
            .validation_chains
            .skipped_async_field_keys_to_clear(field, trigger);

        for key in keys {
            if let Some(validator) = self.validation_chains.field_validator_mut(&key) {
                validator.lifecycle.clear_async_skip();
            }
        }
    }

    fn validate_field_validator_key(
        &mut self,
        key: ValidatorKey,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus> {
        let errors = {
            let validator = self.validation_chains.field_validator(&key)?;
            if !validator.lifecycle.should_run(trigger) {
                return None;
            }
            let validate = validator.validate.as_ref()?;

            let model = self.draft.current();
            let field_metadata = self.field_store.metadata(&key.field);
            let context = ValidatorContext {
                form: model,
                field_identity: key.field.clone(),
                validator_id: key.id,
                source: validator.lifecycle.source().clone(),
                trigger,
                field_metadata,
                submit_intent: self.submit_intent_ref_for_trigger(trigger),
            };

            validate(model, context)
        };
        let submit_intent = self.submit_intent_for_trigger(trigger);
        let validator = self
            .validation_chains
            .field_validator_mut(&key)
            .expect("validator disappeared during synchronous validation");
        let outcome = validator
            .lifecycle
            .replace_errors(trigger, submit_intent, errors);
        let status = outcome.status();

        self.emit_lifecycle_observer_event(ValidationTarget::Field(key.field.clone()), outcome);

        Some(status)
    }

    fn validate_collection_item_field_validator_key(
        &mut self,
        key: ValidatorKey,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus> {
        let template_key = collection_item_template_key_for_field(&key.field, key.id)?;
        let (collection, item, _) = key.field.collection_item_parts()?;
        let collection = FieldIdentity::new(collection);
        let errors = {
            let validator = self
                .validation_chains
                .collection_item_template(&template_key)?;
            let lifecycle = self.validation_chains.collection_item_state(&key)?;
            if !lifecycle.should_run(trigger) {
                return None;
            }
            let collection_state = self.field_store.collection(&collection)?;
            let model = self.draft.current();
            let field_metadata = self.field_store.metadata(&key.field);
            let context = ValidatorContext {
                form: model,
                field_identity: key.field.clone(),
                validator_id: key.id,
                source: lifecycle.source().clone(),
                trigger,
                field_metadata,
                submit_intent: self.submit_intent_ref_for_trigger(trigger),
            };

            (validator.validate)(model, collection_state, item, context)
        };
        let submit_intent = self.submit_intent_for_trigger(trigger);
        let validator = self
            .validation_chains
            .collection_item_state_mut(&key)
            .expect("collection item validator state disappeared during synchronous validation");
        let outcome = validator.replace_errors(trigger, submit_intent, errors);
        let status = outcome.status();

        self.emit_lifecycle_observer_event(ValidationTarget::Field(key.field), outcome);

        Some(status)
    }

    fn validate_form_chain(&mut self, trigger: ValidationTrigger) {
        if !self.validate_form_sync_chain(trigger) {
            self.skip_async_form_validators_for_chain(trigger);
        } else {
            self.clear_skipped_async_form_validators_for_chain(trigger);
        }
    }

    fn validate_form_sync_chain(&mut self, trigger: ValidationTrigger) -> bool {
        let ids = self.validation_chains.sync_form_ids_for_chain(trigger);
        let mut valid = true;

        for id in ids {
            if self.validate_form_validator_id(id, trigger) == Some(ValidationStatus::Invalid) {
                valid = false;
            }
        }

        valid
    }

    fn skip_async_form_validators_for_chain(&mut self, trigger: ValidationTrigger) {
        let mut events = Vec::new();
        let ids = self.validation_chains.async_form_ids_to_skip(trigger);

        let submit_intent = self.submit_intent_for_trigger(trigger);

        for id in ids {
            let Some(validator) = self.validation_chains.form_validator_mut(id) else {
                continue;
            };

            let outcome = validator
                .lifecycle
                .skip_async(trigger, submit_intent.clone());
            events.push((ValidationTarget::Form, outcome));
        }

        for (target, outcome) in events {
            self.emit_lifecycle_observer_event(target, outcome);
        }
    }

    fn clear_skipped_async_form_validators_for_chain(&mut self, trigger: ValidationTrigger) {
        let ids = self
            .validation_chains
            .skipped_async_form_ids_to_clear(trigger);

        for id in ids {
            if let Some(validator) = self.validation_chains.form_validator_mut(id) {
                validator.lifecycle.clear_async_skip();
            }
        }
    }

    fn validate_form_validator_id(
        &mut self,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus> {
        let errors = {
            let validator = self.validation_chains.form_validator(id)?;
            if !validator.lifecycle.should_run(trigger) {
                return None;
            }
            let validate = validator.validate.as_ref()?;

            let context = FormValidatorContext {
                form: self.draft.current(),
                validator_id: id,
                source: validator.lifecycle.source().clone(),
                trigger,
                field_store: &self.field_store,
                submit_intent: self.submit_intent_ref_for_trigger(trigger),
            };

            validate(context)
        };
        let submit_intent = self.submit_intent_for_trigger(trigger);
        let validator = self
            .validation_chains
            .form_validator_mut(id)
            .expect("validator disappeared during synchronous validation");
        let outcome = validator
            .lifecycle
            .replace_errors(trigger, submit_intent, errors);
        let status = outcome.status();

        self.emit_lifecycle_observer_event(ValidationTarget::Form, outcome);

        Some(status)
    }

    fn validation_errors_matching<'a>(
        &'a self,
        include: impl Fn(&ValidationTarget) -> bool,
    ) -> Vec<ValidationErrorView<'a, Error>> {
        let mut errors = Vec::new();

        self.validation_chains
            .append_validation_errors_matching(&mut errors, &include);

        for error in self.submission.errors() {
            if !include(&error.target) {
                continue;
            }

            errors.push(ValidationErrorView {
                target: error.target.clone(),
                source: &error.source,
                validator_id: None,
                error: &error.error,
            });
        }

        errors
    }

    fn validation_errors_matching_for_submit_intent<'a, Intent>(
        &'a self,
        intent: &Intent,
        include: impl Fn(&ValidationTarget) -> bool,
    ) -> Vec<ValidationErrorView<'a, Error>>
    where
        Intent: PartialEq + 'static,
    {
        let mut errors = Vec::new();

        self.validation_chains
            .append_validation_errors_matching_for_submit_intent(&mut errors, intent, &include);

        for error in self.submission.errors() {
            if !include(&error.target) || !self.submit_error_blocks_submit_intent(error, intent) {
                continue;
            }

            errors.push(ValidationErrorView {
                target: error.target.clone(),
                source: &error.source,
                validator_id: None,
                error: &error.error,
            });
        }

        errors
    }

    fn clear_submit_errors(&mut self) {
        self.submission.clear_errors();
    }

    fn clear_submit_errors_for_field(&mut self, field: &FieldIdentity) {
        self.submission
            .retain_errors(|error| error.target.as_field() != Some(field));
    }

    fn store_submit_errors<Intent>(
        &mut self,
        submitted: &SubmissionSnapshot<Model, Intent>,
        submit_errors: SubmitErrors<Model, Error>,
        submit_intent: SubmitIntentSnapshot,
    ) {
        let (source, errors) = submit_errors.into_parts();
        let current = self.draft.current();
        let stored_errors: Vec<_> = errors
            .into_iter()
            .filter_map(|error| {
                if !self.submit_error_applies_to_current(&error, current, submitted) {
                    return None;
                }

                Some(StoredSubmitError {
                    target: error.target,
                    source: source.clone(),
                    submit_intent: Some(submit_intent.clone()),
                    error: error.error,
                })
            })
            .collect();

        self.submission.set_errors(stored_errors);
    }

    fn submit_error_applies_to_current<Intent>(
        &self,
        error: &SubmitError<Model, Error>,
        current: &Model,
        submitted: &SubmissionSnapshot<Model, Intent>,
    ) -> bool {
        if error.applies_to_current.is_some() {
            return error.applies_to(current, submitted.value());
        }

        let Some(field) = error.target.as_field() else {
            return true;
        };

        let current_version = self.field_store.version(field);
        let submitted_version = submitted.field_version(field);

        current_version == submitted_version
    }

    fn should_show_validation_errors(&self, target: &ValidationTarget) -> bool {
        match self.error_visibility_policy {
            ErrorVisibilityPolicy::Always => true,
            ErrorVisibilityPolicy::SubmitOnly => self.submission.attempt_count() > 0,
            ErrorVisibilityPolicy::BlurOrSubmit => {
                if self.submission.attempt_count() > 0 {
                    return true;
                }

                match target {
                    ValidationTarget::Form => false,
                    ValidationTarget::Field(field) => self.field_store.metadata(field).is_blurred(),
                }
            }
            ErrorVisibilityPolicy::TouchedOrSubmit => {
                if self.submission.attempt_count() > 0 {
                    return true;
                }

                match target {
                    ValidationTarget::Form => false,
                    ValidationTarget::Field(field) => self.field_store.metadata(field).is_touched(),
                }
            }
        }
    }

    fn submit_validation_blocker_for_intent<Intent>(&self, intent: &Intent) -> SubmitBlocker
    where
        Intent: PartialEq + 'static,
    {
        if self.has_pending_validation_for_submit_intent(intent)
            || self.has_unresolved_async_validation_for_submit_intent(intent)
        {
            SubmitBlocker::PendingValidation
        } else {
            SubmitBlocker::ValidationErrors
        }
    }
}
