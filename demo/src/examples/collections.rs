use dioform::prelude::*;
use dioxus::prelude::*;

/// A `CollectionBinding` owns a `Vec` field as repeatable rows with
/// library-managed identity: no app-supplied keys. Every mutation is a method
/// on the binding: `append`, `insert`, `remove`, `move_to_index`, `swap`,
/// `replace`, and `clear`. Per-row field state (and later, per-row validation)
/// follows each item through reordering and removal by its identity, not its
/// index.
#[derive(Clone, Debug, PartialEq, Form)]
struct Playlist {
    tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq, Form)]
struct Track {
    title: String,
}

fn sample() -> Playlist {
    Playlist {
        tracks: vec![
            Track {
                title: "Intro".into(),
            },
            Track {
                title: "Verse".into(),
            },
            Track {
                title: "Chorus".into(),
            },
        ],
    }
}

#[component]
pub fn CollectionsExample() -> Element {
    let form = use_form(sample());
    let tracks = form.collection(Playlist::fields().tracks());

    let items = tracks.items();
    let count = items.len();

    let add = tracks.clone();
    let insert_top = tracks.clone();
    let swap_ends = tracks.clone();
    let clear = tracks.clone();

    rsx! {
        div { class: "flex flex-wrap gap-2",
            button {
                class: "btn btn-sm btn-primary",
                onclick: move |_| { add.append(Track { title: String::new() }); },
                "append"
            }
            button {
                class: "btn btn-sm btn-outline",
                onclick: move |_| { insert_top.insert(0, Track { title: "New first".into() }); },
                "insert at top"
            }
            button {
                class: "btn btn-sm btn-outline",
                disabled: count < 2,
                onclick: move |_| { swap_ends.swap(0, count - 1); },
                "swap ends"
            }
            button {
                class: "btn btn-sm btn-ghost",
                disabled: count == 0,
                onclick: move |_| { clear.clear(); },
                "clear"
            }
        }

        ul { class: "mt-4 space-y-2",
            for item in items.iter().cloned() {
                {
                    let index = item.index();
                    let title = item.text(Track::fields().title());
                    let up = tracks.clone();
                    let down = tracks.clone();
                    let replace = tracks.clone();
                    let remove = tracks.clone();
                    let id_up = item.identity();
                    let id_down = item.identity();
                    let id_remove = item.identity();
                    rsx! {
                        li { key: "{index}", class: "flex items-center gap-2",
                            span { class: "w-6 text-right font-mono text-xs text-base-content/50", "{index}" }
                            input {
                                class: "input input-bordered input-sm flex-1",
                                name: title.name(),
                                value: title.value(),
                                oninput: title.oninput(),
                                onblur: title.onblur(),
                            }
                            button {
                                class: "btn btn-xs btn-ghost",
                                disabled: index == 0,
                                onclick: move |_| { up.move_to_index(id_up, index - 1); },
                                "↑"
                            }
                            button {
                                class: "btn btn-xs btn-ghost",
                                disabled: index + 1 == count,
                                onclick: move |_| { down.move_to_index(id_down, index + 1); },
                                "↓"
                            }
                            button {
                                class: "btn btn-xs btn-ghost",
                                onclick: move |_| { replace.replace(index, Track { title: "Replaced".into() }); },
                                "replace"
                            }
                            button {
                                class: "btn btn-xs btn-outline btn-error",
                                onclick: move |_| { remove.remove(id_remove); },
                                "remove"
                            }
                        }
                    }
                }
            }
        }
        p { class: "mt-3 text-sm text-base-content/60", "{count} track(s)" }
    }
}
