use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use xraytsubaki::prelude::*;

use crate::dto::FeffFitResultDto;

pub struct AppState {
    pub group: Mutex<XASGroup>,
    pub fit_results: Mutex<HashMap<String, FeffFitResultDto>>,
    pub workspace_path: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        let mut group = XASGroup::new();

        // Auto-load test data for development
        #[cfg(debug_assertions)]
        {
            let autoload_enabled = std::env::var("XRAYTSUBAKI_AUTOLOAD_DEV_DATA")
                .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
                .unwrap_or(false);

            if autoload_enabled {
                let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
                if let Some(repo_root) = manifest_dir.parent().and_then(|p| p.parent()) {
                    let test_file = repo_root.join("crates/xraytsubaki/tests/testfiles/Ru_QAS.dat");

                    if test_file.exists() {
                        match io::load_spectrum_QAS_trans(&test_file) {
                            Ok(mut spec) => {
                                spec.set_name("Ru_QAS.dat".to_string());
                                // Run full processing pipeline
                                if let Err(e) = spec.find_e0() {
                                    eprintln!("[dev] find_e0 failed: {e}");
                                }
                                if let Err(e) = spec.normalize() {
                                    eprintln!("[dev] normalize failed: {e}");
                                }
                                if let Err(e) = spec.calc_background() {
                                    eprintln!("[dev] calc_background failed: {e}");
                                }
                                if let Err(e) = spec.fft() {
                                    eprintln!("[dev] fft failed: {e}");
                                }
                                group.add_spectrum(spec);
                                eprintln!("[dev] Auto-loaded Ru_QAS.dat with full pipeline");
                            }
                            Err(e) => {
                                eprintln!("[dev] Failed to load Ru_QAS.dat: {e}");
                            }
                        }
                    } else {
                        eprintln!("[dev] XRAYTSUBAKI_AUTOLOAD_DEV_DATA enabled but test file was not found");
                    }
                } else {
                    eprintln!("[dev] XRAYTSUBAKI_AUTOLOAD_DEV_DATA enabled but could not locate repo root");
                }
            }
        }

        Self {
            group: Mutex::new(group),
            fit_results: Mutex::new(HashMap::new()),
            workspace_path: Mutex::new(None),
        }
    }
}
