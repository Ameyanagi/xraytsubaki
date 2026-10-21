//! rexafs-gui: GPUI desktop shell for rexafs XAS analysis.
//!
//! Opens a single window whose root view is [`app::StudioApp`].
//! UX reference: doc/gui-ux-design.md.

// Explorer/Start menu launches must not allocate a console. Keep the test
// harness as a console program so Cargo can collect its output normally.
#![cfg_attr(all(target_os = "windows", not(test)), windows_subsystem = "windows")]

mod accessibility;
mod app;
mod catalog;
mod codex_client;
mod debug_stats;
mod feffgen;
mod fit_details;
mod fit_report;
mod fitting;
mod group_identity;
mod icons;
mod import_mapping;
mod import_recipes;
mod joint_fitting;
mod licenses;
mod params;
mod plot_export;
mod plot_ranges;
mod plotting;
mod project;
mod publication;
mod rmc_fitting;
mod series_measurements;
// Live intake is not connected to the desktop until queue/recovery qualification.
mod analysis_store;
mod fluorescence_history;
mod live;
#[allow(dead_code)]
mod live_intake;
mod normalization_history;
mod peak_fits;
mod series_fits;
mod settings;
mod source_evidence;
mod spectrum_colors;
mod spectrum_interest;
mod storage_cleanup;
mod structure;
mod text;
mod theme;
mod updates;
mod wavelet_history;
mod widgets;

use std::path::PathBuf;

use gpui::{App, AppContext, Bounds, Size, WindowBounds, WindowOptions, px};

use crate::app::StudioApp;

fn main() {
    #[cfg(target_os = "windows")]
    attach_parent_console();

    // Handle re-executed FEFF stage workers before argument or GUI setup.
    // Windows runs FEFF10 through the bundled helper process instead.
    #[cfg(all(feature = "feff10-runner", not(windows)))]
    feff10::worker::init();

    let first_arg = std::env::args_os().nth(1);
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    if first_arg.as_deref() == Some(std::ffi::OsStr::new("--finish-update")) {
        let result = std::env::args_os()
            .nth(2)
            .ok_or("Missing update transaction".into())
            .and_then(|path| updates::install::finish_update(std::path::Path::new(&path)));
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    match first_arg.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("--version") => {
            println!("rexafs {}", updates::installed_label());
            return;
        }
        Some("--build-info") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&updates::build_info()).unwrap()
            );
            return;
        }
        Some("--self-check") => {
            if let Err(error) = check_package() {
                eprintln!("rexafs package check failed: {error}");
                std::process::exit(1);
            }
            return;
        }
        #[cfg(target_os = "macos")]
        Some("--self-check-updater") => {
            if let Err(error) = updates::install::check_helper() {
                eprintln!("rexafs updater check failed: {error}");
                std::process::exit(1);
            }
            println!("rexafs signed updater helper check passed");
            return;
        }
        Some("--self-check-feff") => {
            if let Err(error) = feffgen::check_package_backends() {
                eprintln!("rexafs FEFF package check failed: {error}");
                std::process::exit(1);
            }
            return;
        }
        _ => {}
    }

    // Optional positional arg: a spectrum, folder or .rxs to open. Enables
    // "open with", scripted launches, and screenshot testing without driving
    // the native file dialog.
    let initial_open: Option<PathBuf> = first_arg.map(PathBuf::from);

    // gpui 0.2.2 has no zero-arg `Application::new()`; the public entry point
    // is `gpui_platform::application()`.
    gpui_platform::application()
        .with_assets(icons::IconAssets)
        .run(move |cx: &mut App| {
            cx.bind_keys(widgets::text_input::text_input_keybindings());
            cx.bind_keys(app::studio_keybindings());
            // Quit asks to save unsaved work first (see `confirm_close`). The
            // window is busy while it dispatches this action, so defer.
            cx.on_action(|_: &app::Quit, cx| {
                cx.defer(|cx| {
                    let studio = cx
                        .windows()
                        .into_iter()
                        .find_map(|window| window.downcast::<StudioApp>());
                    let quit = studio.is_none_or(|studio| {
                        studio
                            .update(cx, |app, window, cx| {
                                app.confirm_close(app::CloseIntent::Quit, window, cx)
                            })
                            .unwrap_or(true)
                    });
                    if quit {
                        cx.quit();
                    }
                });
            });
            let display = cx.primary_display().map(|display| display.visible_bounds());
            let window_size = initial_window_size(display.map(|bounds| bounds.size));
            let bounds = display.map_or_else(
                || Bounds::centered(None, window_size, cx),
                |display| Bounds::centered_at(display.center(), window_size),
            );
            let main_window = cx
                .open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        titlebar: Some(gpui::TitlebarOptions {
                            title: Some(updates::application_name().into()),
                            ..Default::default()
                        }),
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(Size {
                            width: px(960.).min(window_size.width),
                            height: px(640.).min(window_size.height),
                        }),
                        app_id: Some(
                            if updates::installed_channel() == updates::UpdateChannel::Nightly {
                                "com.rexafs.nightly".into()
                            } else {
                                "com.rexafs.desktop".into()
                            },
                        ),
                        ..Default::default()
                    },
                    |window, cx| {
                        if debug_stats::enabled() {
                            eprintln!("[renderer] {:?}", window.gpu_specs());
                        }
                        accessibility::install(window, cx);
                        cx.new(|cx| StudioApp::new_with_open(initial_open.clone(), window, cx))
                    },
                )
                .expect("failed to open window");
            let _ = main_window.update(cx, |_, window, _| accessibility::show(window));
            cx.activate(true);
        });
}

/// Leave room for window decorations on laptop screens and scaled desktops.
fn initial_window_size(display: Option<Size<gpui::Pixels>>) -> Size<gpui::Pixels> {
    let preferred = Size {
        width: px(1440.),
        height: px(900.),
    };
    display.map_or(preferred, |size| Size {
        width: preferred.width.min(size.width * 0.95),
        height: preferred.height.min(size.height * 0.9),
    })
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn startup_fits_small_and_scaled_displays() {
        for (width, height) in [(1366., 768.), (1280., 720.), (800., 600.), (3840., 2160.)] {
            let available = Size {
                width: px(width),
                height: px(height),
            };
            let size = initial_window_size(Some(available));
            assert!(size.width < available.width);
            assert!(size.height < available.height);
            assert!(size.width <= px(1440.));
            assert!(size.height <= px(900.));
        }
        assert_eq!(
            initial_window_size(None),
            Size {
                width: px(1440.),
                height: px(900.)
            }
        );
    }
}

/// Keep terminal diagnostics available when launched from a shell, while
/// preserving redirected output used by packaging and installer checks.
#[cfg(target_os = "windows")]
fn attach_parent_console() {
    use windows::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    };

    // SAFETY: these APIs inspect this process's standard handles and attach
    // only to an existing parent console; they never create a console window.
    unsafe {
        let has_output = [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
            .into_iter()
            .any(|kind| GetStdHandle(kind).is_ok_and(|handle| !handle.is_invalid()));
        if !has_output {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
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

fn native_menus(blocked: bool, cx: &mut App) {
    use gpui::{Menu, MenuItem};
    cx.set_menus([
        Menu::new("rexafs").items([
            MenuItem::action("About rexafs", app::ShowHelp).disabled(blocked),
            MenuItem::separator(),
            MenuItem::action("Quit rexafs", app::Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Import…", app::ImportPaths).disabled(blocked),
            MenuItem::action("Open project…", app::OpenProject).disabled(blocked),
            MenuItem::action("Save project…", app::SaveProject).disabled(blocked),
        ]),
        Menu::new("Edit").items([
            MenuItem::action("Undo analysis change", app::Undo).disabled(blocked),
            MenuItem::action("Redo analysis change", app::Redo).disabled(blocked),
            MenuItem::separator(),
            MenuItem::action("Cut", widgets::text_input::Cut),
            MenuItem::action("Copy", widgets::text_input::Copy),
            MenuItem::action("Paste", widgets::text_input::Paste),
            MenuItem::action("Select all", widgets::text_input::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Data", app::StageData).disabled(blocked),
            MenuItem::action("Normalize", app::StageNormalize).disabled(blocked),
            MenuItem::action("Background", app::StageBackground).disabled(blocked),
            MenuItem::action("Transform", app::StageTransform).disabled(blocked),
            MenuItem::action("Fit", app::StageFit).disabled(blocked),
            MenuItem::action("Series", app::StageSeries).disabled(blocked),
            MenuItem::action("Publish", app::StagePublish).disabled(blocked),
            MenuItem::separator(),
            MenuItem::action("Groups", app::ToggleDataPanel).disabled(blocked),
            MenuItem::action("Parameters", app::ToggleContextPanel).disabled(blocked),
            MenuItem::action("Switch theme", app::SwitchTheme).disabled(blocked),
            MenuItem::action("Command palette…", app::PaletteOpen).disabled(blocked),
        ]),
        Menu::new("Help").items([
            MenuItem::action("Open Cu example", app::ShowExample).disabled(blocked),
            MenuItem::action("Synthetic copper reduction…", app::ShowCopperSeries)
                .disabled(blocked),
            MenuItem::action("Licenses", app::ShowLicenses).disabled(blocked),
            MenuItem::action("Storage…", app::ShowStorage).disabled(blocked),
            MenuItem::action("Updates…", app::ShowUpdates).disabled(blocked),
        ]),
    ]);
}
