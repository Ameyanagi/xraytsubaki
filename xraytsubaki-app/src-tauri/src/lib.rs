pub mod commands;
pub mod dto;
pub mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Spectra commands
            commands::spectra::load_spectra,
            commands::spectra::get_spectrum_list,
            commands::spectra::get_spectrum_data,
            commands::spectra::find_e0,
            commands::spectra::normalize,
            commands::spectra::calc_background,
            commands::spectra::fft,
            commands::spectra::run_pipeline,
            commands::spectra::batch_process,
            commands::spectra::remove_spectra,
            // Plotting commands
            commands::plotting::plot_spectrum,
            commands::plotting::plot_group,
            commands::plotting::plot_svg,
            // Workspace commands
            commands::workspace::save_workspace,
            commands::workspace::load_workspace,
            commands::workspace::get_workspace_path,
            // Fitting commands
            commands::fitting::run_feff_paths,
            commands::fitting::run_feff_fit,
            commands::fitting::get_fit_result,
            commands::fitting::list_fit_results,
            commands::fitting::plot_fit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
