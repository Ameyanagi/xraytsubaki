//! xraytsubaki-gui: GPUI desktop shell for xraytsubaki XAS analysis.
//!
//! Opens a single window whose root view is [`app::StudioApp`].
//! UX reference: doc/gui-ux-design.md.

mod app;
mod catalog;
mod debug_stats;
mod feffgen;
mod fitting;
mod params;
mod plotting;
mod project;
mod theme;
mod widgets;

use std::path::PathBuf;

use gpui::{App, AppContext, Bounds, Size, WindowBounds, WindowOptions, px};

use crate::app::StudioApp;

fn main() {
    // FEFFRS uses re-executed worker processes for its bundled FEFF stages.
    // This is compiled only when that optional backend is included.
    #[cfg(feature = "feff10-runner")]
    feff10::worker::init();

    // Optional positional arg: a data file to auto-load on launch. Enables
    // "open with", scripted launches, and screenshot testing without driving
    // the native file dialog.
    let initial_open: Option<PathBuf> = std::env::args().nth(1).map(PathBuf::from);

    // gpui 0.2.2 has no zero-arg `Application::new()`; the public entry point
    // is `gpui_platform::application()`.
    gpui_platform::application().run(move |cx: &mut App| {
        cx.bind_keys(widgets::text_input::text_input_keybindings());
        cx.bind_keys(app::studio_keybindings());
        let window_size = Size {
            width: px(1440.0),
            height: px(900.0),
        };
        let bounds = Bounds::centered(None, window_size, cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| StudioApp::new_with_open(initial_open.clone(), window, cx)),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}
