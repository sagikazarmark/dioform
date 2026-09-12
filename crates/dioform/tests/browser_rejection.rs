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

#[test]
fn restored_parser_can_read_parse_state_during_registration() {
    let form: FormHandle<Model> =
        FormHandle::from_config(FormConfig::new(response()).browser_rejection((), |_| {
            BrowserRejection::new(SubmitErrors::none()).raw_field(Model::fields().count(), "abc")
        }));
    let reader = form.clone();
    let binding = form.parsed_text_with(
        Model::fields().count(),
        move |raw| {
            assert!(reader.parse_errors().is_empty());
            raw.parse::<u32>()
        },
        u32::to_string,
    );
    assert_eq!(binding.value(), "abc");
    assert_eq!(form.parse_errors().len(), 1);
}

#[test]
fn restored_parser_can_read_parse_state_during_restoration() {
    let form = FormHandle::new(response());
    let reader = form.clone();
    let binding = form.parsed_text_with(
        Model::fields().count(),
        move |raw| {
            assert!(reader.parse_errors().is_empty());
            raw.parse::<u32>()
        },
        u32::to_string,
    );
    form.restore_browser_rejection(response(), (), |_| {
        BrowserRejection::new(SubmitErrors::none()).raw_field(Model::fields().count(), "abc")
    });
    assert_eq!(binding.value(), "abc");
    assert_eq!(form.parse_errors().len(), 1);
}

#[test]
fn restored_parser_cannot_reinstall_raw_text_after_reentrant_reset_or_write() {
    for reset in [true, false] {
        let form = FormHandle::new(response());
        let writer = form.clone();
        let binding = form.parsed_text_with(
            Model::fields().count(),
            move |raw| {
                if reset {
                    writer.reset();
                } else {
                    writer.set_field(Model::fields().count(), 9);
                }
                raw.parse::<u32>()
            },
            u32::to_string,
        );
        form.restore_browser_rejection(response(), (), |_| {
            BrowserRejection::new(SubmitErrors::none()).raw_field(Model::fields().count(), "abc")
        });
        assert_eq!(binding.value(), if reset { "7" } else { "9" });
        assert!(form.parse_errors().is_empty());
    }
}

#[test]
fn reentrant_restoration_supersedes_the_entire_outer_raw_input_batch() {
    let form = FormHandle::new(response());
    let writer = form.clone();
    let count = form.parsed_text_with(
        Model::fields().count(),
        move |raw| {
            if raw == "outer count" {
                writer.restore_browser_rejection(response(), (), |_| {
                    BrowserRejection::new(SubmitErrors::none())
                        .raw_field(Model::fields().count(), "inner count")
                        .raw_field(Model::fields().name(), "inner name")
                });
            }
            raw.parse::<u32>()
        },
        u32::to_string,
    );
    let parsed_names = Rc::new(RefCell::new(Vec::new()));
    let calls = parsed_names.clone();
    let name = form.parsed_text_with(
        Model::fields().name(),
        move |raw| {
            calls.borrow_mut().push(raw.to_owned());
            Err::<String, _>("invalid")
        },
        String::clone,
    );
    form.restore_browser_rejection(response(), (), |_| {
        BrowserRejection::new(SubmitErrors::none())
            .raw_field(Model::fields().count(), "outer count")
            .raw_field(Model::fields().name(), "outer name")
    });
    assert_eq!(count.value(), "inner count");
    assert_eq!(name.value(), "inner name");
    assert_eq!(&*parsed_names.borrow(), &["inner name"]);
    assert_eq!(form.parse_errors().len(), 2);
}

#[derive(Clone, Debug, PartialEq, Form)]
struct Rows {
    rows: Vec<Model>,
}

#[test]
fn collection_insertion_retires_deferred_collection_text() {
    let form = FormHandle::new(Rows {
        rows: vec![response()],
    });
    form.restore_browser_rejection(form.snapshot(), (), |_| {
        BrowserRejection::new(SubmitErrors::none())
            .raw_field(Rows::fields().rows(), "old invalid response")
    });
    form.collection(Rows::fields().rows())
        .append_programmatic(response());
    let binding = form.parsed_text_with(
        Rows::fields().rows(),
        |_| Err::<Vec<Model>, _>("invalid rows"),
        |rows| rows.len().to_string(),
    );
    assert_eq!(binding.value(), "2");
    assert!(form.parse_errors().is_empty());
}

#[derive(Clone, Debug, PartialEq, Form)]
struct NestedRows {
    child: Rows,
}

#[test]
fn collection_mutations_retire_containing_text_and_preserve_unaffected_item_text() {
    for operation in [
        "append", "move", "swap", "reorder", "replace", "remove", "clear",
    ] {
        let form = FormHandle::new(NestedRows {
            child: Rows {
                rows: vec![response(), response()],
            },
        });
        let rows = NestedRows::fields().child().join(Rows::fields().rows());
        form.restore_browser_rejection(form.snapshot(), (), |targets| {
            let first = targets
                .collection_item_field(rows.clone(), 0, Model::fields().count())
                .unwrap();
            let second = targets
                .collection_item_field(rows.clone(), 1, Model::fields().count())
                .unwrap();
            BrowserRejection::new(SubmitErrors::none())
                .raw_field(rows.clone(), "old collection")
                .raw_field(NestedRows::fields().child(), "old parent")
                .raw_field_identity(first, "first invalid")
                .raw_field_identity(second, "second invalid")
        });
        let collection = form.collection(rows.clone());
        let first = collection.items()[0].clone();
        let second = collection.items()[1].clone();
        match operation {
            "append" => {
                collection.append_programmatic(response());
            }
            "move" => {
                assert!(collection.move_to_index_programmatic(first.identity(), 1));
            }
            "swap" => {
                assert!(collection.swap_programmatic(0, 1));
            }
            "reorder" => {
                assert!(collection.reorder_programmatic(&[second.identity(), first.identity()]));
            }
            "replace" => {
                assert!(collection.replace_programmatic(0, response()));
            }
            "remove" => {
                assert!(collection.remove_programmatic(first.identity()).is_some());
            }
            "clear" => {
                assert!(collection.clear_programmatic());
            }
            _ => unreachable!(),
        }
        let root = form.parsed_text_with(
            rows,
            |_| Err::<Vec<Model>, _>("invalid collection"),
            |_| "current collection".to_owned(),
        );
        let parent = form.parsed_text_with(
            NestedRows::fields().child(),
            |_| Err::<Rows, _>("invalid parent"),
            |_| "current parent".to_owned(),
        );
        assert_eq!(root.value(), "current collection", "{operation}");
        assert_eq!(parent.value(), "current parent", "{operation}");
        assert!(form.parse_errors().is_empty(), "{operation}");
        if operation != "clear" {
            assert_eq!(
                second.number(Model::fields().count()).value(),
                "second invalid"
            );
            if operation == "replace" {
                assert_eq!(first.number(Model::fields().count()).value(), "7");
            } else if operation != "remove" {
                assert_eq!(
                    first.number(Model::fields().count()).value(),
                    "first invalid"
                );
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Form)]
struct Nested {
    child: Model,
}

#[test]
fn item_field_writes_retire_containing_text_even_during_restored_parsing() {
    for during_parser in [false, true] {
        let form = FormHandle::new(Rows {
            rows: vec![response(), response()],
        });
        form.restore_browser_rejection(form.snapshot(), (), |targets| {
            let second = targets
                .collection_item_field(Rows::fields().rows(), 1, Model::fields().count())
                .unwrap();
            BrowserRejection::new(SubmitErrors::none())
                .raw_field(Rows::fields().rows(), "old collection")
                .raw_field_identity(second, "unaffected item")
        });
        let collection = form.collection(Rows::fields().rows());
        let first = collection.items()[0].clone();
        if !during_parser {
            first.select(Model::fields().count()).set_value(9);
        }
        let binding = form.parsed_text_with(
            Rows::fields().rows(),
            move |_| {
                if during_parser {
                    first.select(Model::fields().count()).set_value(9);
                }
                Err::<Vec<Model>, _>("invalid collection")
            },
            |rows| rows[0].count.to_string(),
        );
        assert_eq!(binding.value(), "9", "during parser: {during_parser}");
        assert!(form.parse_errors().is_empty());
        assert_eq!(
            collection.items()[1]
                .number(Model::fields().count())
                .value(),
            "unaffected item"
        );
    }
}

#[test]
fn collection_parser_cannot_reinstall_text_after_reentrant_insertion() {
    let form = FormHandle::new(Rows {
        rows: vec![response()],
    });
    let writer = form.clone();
    let binding = form.parsed_text_with(
        Rows::fields().rows(),
        move |_| {
            writer
                .collection(Rows::fields().rows())
                .append_programmatic(response());
            Err::<Vec<Model>, _>("invalid collection")
        },
        |rows| rows.len().to_string(),
    );
    form.restore_browser_rejection(form.snapshot(), (), |_| {
        BrowserRejection::new(SubmitErrors::none())
            .raw_field(Rows::fields().rows(), "old collection")
    });
    assert_eq!(binding.value(), "2");
    assert!(form.parse_errors().is_empty());
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
