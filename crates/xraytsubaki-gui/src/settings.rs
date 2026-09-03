//! Machine-local user settings (`~/.xraytsubaki/settings.json`): things that
//! belong to this computer and this user rather than to a project — the CIF
//! library folder, the AMCSD database file and the Materials Project API key.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    /// Folder scanned for `*.cif` files (the "CIF library" structure source).
    pub cif_library: Option<PathBuf>,
    /// Local copy of the AMCSD SQLite database.
    pub amcsd_db: Option<PathBuf>,
    /// Materials Project API key (kept out of project files on purpose).
    pub mp_api_key: String,
}

/// `~/.xraytsubaki` (created on demand).
pub fn app_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dir = PathBuf::from(home).join(".xraytsubaki");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn settings_path() -> Option<PathBuf> {
    app_dir().map(|d| d.join("settings.json"))
}

/// Default location for a downloaded AMCSD database.
pub fn default_amcsd_path() -> Option<PathBuf> {
    app_dir().map(|d| d.join("amcsd_cif2.db"))
}

impl UserSettings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|p| Self::load_from(&p).ok())
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub fn save(&self) -> Result<(), String> {
        let path = settings_path().ok_or_else(|| "HOME not set".to_string())?;
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_defaults() {
        let dir = std::env::temp_dir().join(format!("xts-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let s = UserSettings {
            cif_library: Some(PathBuf::from("/tmp/cifs")),
            amcsd_db: None,
            mp_api_key: "abc".into(),
        };
        s.save_to(&path).unwrap();
        assert_eq!(UserSettings::load_from(&path).unwrap(), s);
        // Missing keys fall back to defaults.
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(
            UserSettings::load_from(&path).unwrap(),
            UserSettings::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
