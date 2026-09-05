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

/// `~/.xraytsubaki` (created on demand, owner-only on Unix because it
/// holds `settings.json` with the Materials Project API key).
pub fn app_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dir = PathBuf::from(home).join(".xraytsubaki");
    std::fs::create_dir_all(&dir).ok()?;
    restrict_to_owner(&dir, 0o700);
    Some(dir)
}

/// Best-effort `chmod` on Unix; a no-op elsewhere. Settings hold a
/// credential (the Materials Project API key) in plain text, so the file
/// and its directory must not be readable by other local accounts.
fn restrict_to_owner(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
}

/// `~/.xraytsubaki/settings.json`, or the file named by `XTS_SETTINGS`
/// (scripted launches keep the user's real settings untouched).
fn settings_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("XTS_SETTINGS") {
        return Some(PathBuf::from(p));
    }
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

    /// Write the settings atomically with owner-only permissions (0600); the
    /// API key is stored in plain text, so the file must never be world-
    /// or group-readable. The key is never logged.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        // Unpredictable temp name + exclusive create: a pre-placed file or
        // symlink at the temp path makes the open fail instead of being
        // followed, so the key is never written through someone else's link.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = path.with_extension(format!("json.{}.{nonce:x}.tmp", std::process::id()));
        write_exclusive(&tmp, &text)?;
        restrict_to_owner(&tmp, 0o600);
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        restrict_to_owner(path, 0o600);
        Ok(())
    }
}

/// Create `tmp` exclusively (fails if anything, including a symlink, already
/// exists there), owner-only on Unix, and write `text` into it.
fn write_exclusive(tmp: &Path, text: &str) -> Result<(), String> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(tmp)
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    use std::io::Write;
    if let Err(e) = f.write_all(text.as_bytes()) {
        drop(f);
        let _ = std::fs::remove_file(tmp);
        return Err(e.to_string());
    }
    Ok(())
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "settings file must be owner-only");
        }
        // Missing keys fall back to defaults.
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(
            UserSettings::load_from(&path).unwrap(),
            UserSettings::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn temp_file_is_created_exclusively_and_symlinks_are_not_followed() {
        let dir = std::env::temp_dir().join(format!("xts-settings-excl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tmp = dir.join("a.tmp");
        write_exclusive(&tmp, "one").unwrap();
        assert!(
            write_exclusive(&tmp, "two").is_err(),
            "second create must fail"
        );
        assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "one");
        #[cfg(unix)]
        {
            let target = dir.join("victim");
            std::fs::write(&target, "").unwrap();
            let link = dir.join("b.tmp");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(
                write_exclusive(&link, "secret").is_err(),
                "symlink must not be followed"
            );
            assert_eq!(std::fs::read_to_string(&target).unwrap(), "");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
