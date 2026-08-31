//! Presentation components used by the demo application.

mod code;
mod common;
mod examples;
mod layout;
mod nav;

/// daisyUI components vendored from the `dioxus-daisyui-components` registry by
/// `dx components add`. Generated code: change it upstream, not here.
///
/// Each component ships its full set of daisyUI axes and the demo only reaches
/// for some of them, hence the `dead_code` allowance.
#[allow(dead_code)]
pub mod daisyui;

// The registry writes its cross-component imports as `crate::components::<name>`
// and `dx components add` copies the sources verbatim, so the vendored modules
// have to be reachable at that path too, not only under `daisyui`.
pub use daisyui::*;

pub use code::*;
pub use common::*;
pub use examples::*;
pub use layout::*;
pub use nav::*;
