//! rexafs-gui: GPUI desktop shell for rexafs XAS analysis.
//!
//! Opens a single window whose root view is [`app::StudioApp`].
//! UX reference: doc/gui-ux-design.md.

mod app;
mod catalog;
mod codex_client;
mod debug_stats;
mod feffgen;
mod fit_details;
mod fitting;
mod joint_fitting;
mod params;
mod plotting;
mod project;
mod publication;
mod settings;
mod spectrum_interest;
mod structure;
mod theme;
mod widgets;

use std::path::PathBuf;

use gpui::{App, AppContext, Bounds, Size, WindowBounds, WindowOptions, px};

use crate::app::StudioApp;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--version") => {
            println!("rexafs {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--self-check") => {
            if let Err(error) = check_package() {
                eprintln!("rexafs package check failed: {error}");
                std::process::exit(1);
            }
            return;
        }
        _ => {}
    }
    // FEFFRS uses re-executed worker processes for its bundled FEFF stages.
    // This is compiled only when that optional backend is included.
    #[cfg(feature = "feff10-runner")]
    feff10::worker::init();

    // Optional positional arg: a spectrum, folder or .rxs to open. Enables
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
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("rexafs".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| StudioApp::new_with_open(initial_open.clone(), window, cx)),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}

/// Exercise the distributed example and numerical pipeline without a display.
/// This deliberately requires packaged resources, with no checkout fallback.
fn check_package() -> Result<(), String> {
    let path = app::packaged_data_file().ok_or("Packaged example is missing")?;
    let (energy, mu) = params::load_raw(&path, &params::PipelineParams::default())?;
    let mut spectrum = rexafs::Spectrum::from_arrays(&energy, &mu).map_err(|e| e.to_string())?;
    spectrum.fft().map_err(|e| e.to_string())?;
    let e0 = spectrum.e0().ok_or("Missing E0")?;
    let k = spectrum.k().ok_or("Missing k")?;
    let r = spectrum.r().ok_or("Missing R")?;
    let chi = spectrum.chi().ok_or("Missing chi")?;
    let magnitude = spectrum.chir_mag().ok_or("Missing Fourier magnitude")?;
    if !e0.is_finite()
        || k.is_empty()
        || r.is_empty()
        || chi.iter().chain(magnitude.iter()).any(|v| !v.is_finite())
    {
        return Err("Packaged example produced invalid numerical output".into());
    }
    println!(
        "rexafs {}: package check passed (E0={}, k={}, R={})",
        env!("CARGO_PKG_VERSION"),
        e0,
        k.len(),
        r.len()
    );
    Ok(())
}
