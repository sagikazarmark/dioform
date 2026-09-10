use dioform::prelude::*;
use dioxus::prelude::{Element, Event, VNode, VirtualDom};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Debug, PartialEq, Form)]
struct Model {
    count: u32,
    name: String,
}

fn response() -> Model {
    Model {
        count: 7,
        name: "response".into(),
    }
}

#[derive(Clone, Debug, PartialEq, Form)]
struct Rows {
    rows: Vec<Model>,
}

#[derive(Clone, Debug, PartialEq, Form)]
struct Nested {
    child: Model,
}

#[test]
fn ancestor_write_retires_deferred_text_and_valid_raw_never_changes_response_values() {
    let form = FormHandle::new(Nested { child: response() });
    let count = Nested::fields().child().join(Model::fields().count());
    form.restore_browser_rejection(Nested { child: response() }, (), |_| {
        BrowserRejection::new(SubmitErrors::none()).raw_field(count.clone(), "abc")
    });
    form.set_field(
        Nested::fields().child(),
        Model {
            count: 9,
            name: "edited".into(),
        },
    );
    assert_eq!(form.number(count.clone()).value(), "9");
    form.restore_browser_rejection(Nested { child: response() }, (), |_| {
        BrowserRejection::new(SubmitErrors::none()).raw_field(count.clone(), "0009")
    });
    let first = form.number(count.clone());
    assert_eq!(first.value(), "7");
    assert_eq!(form.snapshot().child.count, 7);
    assert!(form.parse_errors().is_empty());
    form.restore_browser_rejection(Nested { child: response() }, (), |_| {
        BrowserRejection::new(SubmitErrors::none()).raw_field(count.clone(), "abc")
    });
    let second = form.number(count);
    assert_eq!(first.value(), "abc");
    assert_eq!(second.value(), "7");
    form.reset();
    assert_eq!(first.value(), "7");
    assert!(form.parse_errors().is_empty());
}

#[test]
fn collection_restoration_uses_receiving_items_and_raw_text_follows_reordering() {
    let form = FormHandle::new(Rows {
        rows: vec![response()],
    });
    let retired = form.collection(Rows::fields().rows()).items()[0].identity();
    form.restore_browser_rejection(
        Rows {
            rows: vec![response(), response()],
        },
        (),
        |targets| {
            let item = targets.collection_item(Rows::fields().rows(), 0).unwrap();
            let count = targets
                .collection_item_field(Rows::fields().rows(), 0, Model::fields().count())
                .unwrap();
            BrowserRejection::new(SubmitErrors::new([
                SubmitError::field_identity(item, "row rejected".into()),
                SubmitError::field_identity(count.clone(), "count rejected".into()),
            ]))
            .raw_field_identity(count, "abc")
        },
    );
    let collection = form.collection(Rows::fields().rows());
    let first = collection.items()[0].clone();
    assert_ne!(first.identity(), retired);
    assert!(collection.move_to_index_programmatic(first.identity(), 1));
    assert_eq!(first.visible_validation_errors().len(), 1);
    let binding = first.number(Model::fields().count());
    assert_eq!(binding.value(), "abc");
    assert_eq!(binding.visible_validation_errors().len(), 1);
    assert_eq!(
        collection.items()[0]
            .number(Model::fields().count())
            .value(),
        "7"
    );
    drop(binding);

    form.restore_browser_rejection(
        Rows {
            rows: vec![response()],
        },
        (),
        |targets| {
            let count = targets
                .collection_item_field(Rows::fields().rows(), 0, Model::fields().count())
                .unwrap();
            BrowserRejection::new(SubmitErrors::none()).raw_field_identity(count, "removed")
        },
    );
    let removed = collection.items()[0].clone();
    collection.remove_programmatic(removed.identity());
    collection.append_programmatic(response());
    assert_eq!(removed.number(Model::fields().count()).value(), "");
    assert_eq!(
        collection.items()[0]
            .number(Model::fields().count())
            .value(),
        "7"
    );
    assert!(form.parse_errors().is_empty());
}

#[test]
fn related_writes_retire_deferred_raw_but_unrelated_writes_preserve_it() {
    let form = FormHandle::new(response());
    let restore = || {
        form.restore_browser_rejection(response(), (), |_| {
            BrowserRejection::new(SubmitErrors::none()).raw_field(Model::fields().count(), "abc")
        })
    };
    restore();
    form.set_field(Model::fields().name(), "other".into());
    assert_eq!(form.number(Model::fields().count()).value(), "abc");
    restore();
    form.set_field(Model::fields().count(), 9);
    assert_eq!(form.number(Model::fields().count()).value(), "9");
    restore();
    form.reset();
    assert_eq!(form.number(Model::fields().count()).value(), "7");
    restore();
    form.reinitialize(response());
    assert_eq!(form.number(Model::fields().count()).value(), "7");
}

#[test]
fn writing_collection_descendant_before_mounting_retires_its_raw_response() {
    let form = FormHandle::new(Rows {
        rows: vec![response()],
    });
    form.restore_browser_rejection(
        Rows {
            rows: vec![response()],
        },
        (),
        |targets| {
            let count = targets
                .collection_item_field(Rows::fields().rows(), 0, Model::fields().count())
                .unwrap();
            BrowserRejection::new(SubmitErrors::none()).raw_field_identity(count, "abc")
        },
    );
    let item = form.collection(Rows::fields().rows()).items()[0].clone();
    item.select(Model::fields().count()).set_value(9);
    assert_eq!(item.number(Model::fields().count()).value(), "9");
}

#[test]
fn replacing_whole_item_retires_its_deferred_descendant_text() {
    let form = FormHandle::new(Rows {
        rows: vec![response()],
    });
    form.restore_browser_rejection(
        Rows {
            rows: vec![response()],
        },
        (),
        |targets| {
            let count = targets
                .collection_item_field(Rows::fields().rows(), 0, Model::fields().count())
                .unwrap();
            BrowserRejection::new(SubmitErrors::none()).raw_field_identity(count, "abc")
        },
    );
    form.collection(Rows::fields().rows()).replace_programmatic(
        0,
        Model {
            count: 9,
            name: "new".into(),
        },
    );
    assert_eq!(
        form.collection(Rows::fields().rows()).items()[0]
            .number(Model::fields().count())
            .value(),
        "9"
    );
}

#[test]
fn mounted_parse_blocker_and_failing_client_rules_do_not_prevent_restoration() {
    let form = FormHandle::from_config(
        FormConfig::new(response())
            .validation_mode(ValidationMode::submit_then_revalidate())
            .field_validator(Model::fields().name(), "client")
            .check(|_, _| vec!["client".to_owned()]),
    );
    let number = form.number(Model::fields().count());
    number.on_input("old invalid");
    form.restore_browser_rejection(response(), (), |_| {
        BrowserRejection::new(SubmitErrors::new([SubmitError::form(
            "server only".to_owned(),
        )]))
        .raw_field(Model::fields().count(), "response invalid")
    });
    assert_eq!(number.value(), "response invalid");
    assert!(!number.is_touched());
    assert!(!form.is_dirty());
    assert!(!form.validate_initialization());
    assert_eq!(form.visible_validation_errors().len(), 2);
    assert_eq!(form.parse_errors().len(), 1);
    number.set_value(8);
    form.text(Model::fields().name()).on_input("changed");
    assert_eq!(
        form.visible_field_validation_errors(Model::fields().name())
            .len(),
        1
    );
    assert_eq!(form.visible_validation_errors().len(), 2);
    assert_eq!(
        form.progressive_submit()
            .on_submit(Event::new(Rc::new(()), true)),
        ProgressiveSubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert_eq!(form.validation_errors().len(), 1);
}

#[derive(Default)]
struct Probe {
    handle: RefCell<Option<FormHandle<Model>>>,
    snapshots: RefCell<Vec<PresentationSnapshot>>,
    mappings: Cell<usize>,
    submissions: Cell<usize>,
}

type PresentationSnapshot = (Model, String, usize, u64, Option<SubmitStatus>);

#[test]
fn advanced_core_writes_retire_only_related_deferred_input() {
    let form = FormHandle::new(response());
    let restore = || {
        form.restore_browser_rejection(response(), (), |_| {
            BrowserRejection::new(SubmitErrors::none()).raw_field(Model::fields().count(), "abc")
        })
    };
    restore();
    form.write_advanced(|core| core.set_field(Model::fields().name(), "unrelated".into()));
    assert_eq!(form.number(Model::fields().count()).value(), "abc");
    restore();
    form.write_advanced(|core| core.set_field(Model::fields().count(), 9));
    assert_eq!(form.number(Model::fields().count()).value(), "9");
    restore();
    form.write_advanced(|core| core.reset());
    assert_eq!(form.number(Model::fields().count()).value(), "7");
    restore();
    form.write_advanced(|core| core.reinitialize(response()));
    assert_eq!(form.number(Model::fields().count()).value(), "7");
    restore();
    form.write_advanced(|core| core.reset_field(Model::fields().count()));
    assert_eq!(form.number(Model::fields().count()).value(), "7");
    let snapshot = FormHandle::new(Model {
        count: 12,
        name: "snapshot".into(),
    })
    .state_snapshot();
    restore();
    form.write_advanced(|core| core.restore_state_snapshot(snapshot))
        .unwrap();
    assert_eq!(form.number(Model::fields().count()).value(), "12");
}

#[test]
fn resetting_collection_retires_deferred_input_for_retained_baseline_items() {
    let form = FormHandle::new(Rows {
        rows: vec![response()],
    });
    form.restore_browser_rejection(
        Rows {
            rows: vec![response()],
        },
        (),
        |targets| {
            let count = targets
                .collection_item_field(Rows::fields().rows(), 0, Model::fields().count())
                .unwrap();
            BrowserRejection::new(SubmitErrors::none()).raw_field_identity(count, "abc")
        },
    );
    form.reset_field(Rows::fields().rows());
    assert_eq!(
        form.collection(Rows::fields().rows()).items()[0]
            .number(Model::fields().count())
            .value(),
        "7"
    );
}

fn restored_component(probe: Rc<Probe>) -> Element {
    let mapping_probe = probe.clone();
    let form = use_form_config(FormConfig::new(response()).browser_rejection((), move |_| {
        mapping_probe.mappings.set(mapping_probe.mappings.get() + 1);
        BrowserRejection::new(SubmitErrors::new([SubmitError::form("server".into())]))
            .raw_field(Model::fields().count(), "abc")
    }));
    let listener_probe = probe.clone();
    use_submit_listener(form.clone(), move |_| {
        listener_probe
            .submissions
            .set(listener_probe.submissions.get() + 1)
    });
    let number = use_number(&form, Model::fields().count());
    probe.snapshots.borrow_mut().push((
        form.snapshot(),
        number.value(),
        form.visible_validation_errors().len(),
        form.submit_attempt_count(),
        form.last_submit_status(),
    ));
    probe.handle.replace(Some(form));
    VNode::empty()
}

#[test]
fn independent_initial_renders_match_and_lifecycle_changes_do_not_replay_restoration() {
    let server = Rc::new(Probe::default());
    let client = Rc::new(Probe::default());
    let mut server_dom = VirtualDom::new_with_props(restored_component, server.clone());
    let mut client_dom = VirtualDom::new_with_props(restored_component, client.clone());
    server_dom.rebuild_in_place();
    client_dom.rebuild_in_place();
    assert_eq!(*server.snapshots.borrow(), *client.snapshots.borrow());
    assert_eq!(
        client.snapshots.borrow()[0],
        (response(), "abc".into(), 1, 1, Some(SubmitStatus::Rejected))
    );
    let form = client.handle.borrow().as_ref().unwrap().clone();
    form.reset();
    client_dom.render_immediate_to_vec();
    assert_eq!(client.mappings.get(), 1);
    assert_eq!(
        client.snapshots.borrow().last().unwrap(),
        &(response(), "7".into(), 0, 0, None)
    );
    form.restore_browser_rejection(response(), (), |_| {
        BrowserRejection::new(SubmitErrors::new([SubmitError::form("second".into())]))
            .raw_field(Model::fields().count(), "second raw")
    });
    client_dom.render_immediate_to_vec();
    assert_eq!(
        client.snapshots.borrow().last().unwrap(),
        &(
            response(),
            "second raw".into(),
            1,
            1,
            Some(SubmitStatus::Rejected)
        )
    );
    assert_eq!(client.mappings.get(), 1);
    assert_eq!(client.submissions.get(), 0);
}

#[test]
fn deferred_raw_rejection_mounts_once_without_interaction_and_preserves_server_errors() {
    let form = FormHandle::from_config(FormConfig::new(response()).browser_rejection((), |_| {
        BrowserRejection::new(SubmitErrors::new([
            SubmitError::field(Model::fields().count(), "server count".to_owned()),
            SubmitError::form("server only".to_owned()),
        ]))
        .raw_field(Model::fields().count(), "abc")
    }));
    assert_eq!(form.submit_attempt_count(), 1);
    assert_eq!(form.visible_validation_errors().len(), 2);
    assert!(form.parse_errors().is_empty());
    let binding = form.number(Model::fields().count());
    assert_eq!(binding.value(), "abc");
    assert_eq!(form.snapshot().count, 7);
    assert!(!binding.is_touched());
    assert!(!binding.is_blurred());
    assert_eq!(form.parse_errors().len(), 1);
    assert_eq!(
        form.progressive_submit()
            .on_submit(Event::new(Rc::new(()), true)),
        ProgressiveSubmitResult::Blocked(SubmitBlocker::ParseErrors)
    );
    assert_eq!(form.visible_validation_errors().len(), 2);
    drop(binding);
    let remounted = form.number(Model::fields().count());
    assert_eq!(remounted.value(), "7");
    assert!(form.parse_errors().is_empty());
    assert_eq!(
        form.progressive_submit()
            .on_submit(Event::new(Rc::new(()), true)),
        ProgressiveSubmitResult::Allowed
    );
    assert!(form.validation_errors().is_empty());
}
