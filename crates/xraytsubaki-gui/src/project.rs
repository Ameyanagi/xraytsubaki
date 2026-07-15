//! Project persistence (.xtproj): JSON capturing the data source, pipeline
//! parameters, and fit model so a session can be reopened. Catalog contents
//! are re-scanned on load (indexing is fast); processed data is recomputed
//! through the fingerprint cache.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::fitting::{FitPathSpec, FitRanges, FitVarSpec};
use crate::params::DerivedSpectrum;
use crate::params::PipelineParams;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectFile {
    pub version: u32,
    /// Root folder of the catalog (re-scanned on open).
    pub source_dir: Option<PathBuf>,
    pub params: PipelineParams,
    pub fit_paths: Vec<FitPathSpec>,
    pub fit_vars: Vec<FitVarSpec>,
    pub fit_ranges: FitRanges,
    pub feff_workspace: Option<PathBuf>,
    pub derived: Vec<DerivedSpectrum>,
}

pub const PROJECT_VERSION: u32 = 1;

pub fn save(path: &Path, project: &ProjectFile) -> Result<(), String> {
    let json = serde_json::to_string_pretty(project).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn load(path: &Path) -> Result<ProjectFile, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let project = ProjectFile {
            version: PROJECT_VERSION,
            source_dir: Some(PathBuf::from("/tmp/xts-operando")),
            params: PipelineParams {
                rbkg: Some(1.2),
                fft_kweight: Some(3.0),
                ..Default::default()
            },
            fit_paths: vec![FitPathSpec {
                file: PathBuf::from("/tmp/feff0001.dat"),
                label: "feff0001.dat".into(),
                s02: "amp".into(),
                e0: "de0".into(),
                sigma2: "sig2_1".into(),
                deltar: "dr_1*1.02".into(),
                enabled: true,
            }],
            fit_vars: vec![FitVarSpec {
                name: "amp".into(),
                value: 0.85,
                vary: true,
                min: Some(0.0),
                max: Some(1.5),
                expr: None,
            }],
            fit_ranges: FitRanges::default(),
            feff_workspace: None,
            derived: Vec::new(),
        };
        let path = std::env::temp_dir().join("xts-test.xtproj");
        save(&path, &project).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.params.rbkg, Some(1.2));
        assert_eq!(loaded.fit_paths[0].deltar, "dr_1*1.02");
        assert_eq!(loaded.fit_vars[0].value, 0.85);
        assert_eq!(loaded.fit_vars[0].min, Some(0.0));
        assert_eq!(loaded.fit_vars[0].max, Some(1.5));
        assert_eq!(loaded.source_dir, project.source_dir);
    }

    #[test]
    fn older_fit_variables_load_without_bounds() {
        let var: FitVarSpec =
            serde_json::from_str(r#"{"name":"amp","value":0.85,"vary":true,"expr":null}"#).unwrap();
        assert_eq!(var.min, None);
        assert_eq!(var.max, None);
    }
}
