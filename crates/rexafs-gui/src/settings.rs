//! Machine-local user settings (`~/.rexafs/settings.json`): things that
//! belong to this computer and this user rather than to a project — the CIF
//! library folder, the AMCSD database file and the Materials Project API key.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Prefer the public brand's environment variables; retain codename aliases.
pub fn env_var_os(suffix: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(format!("REXAFS_{suffix}"))
        .or_else(|| std::env::var_os(format!("XTS_{suffix}")))
}

pub fn env_var(suffix: &str) -> Result<String, std::env::VarError> {
    env_var_os(suffix)
        .ok_or(std::env::VarError::NotPresent)?
        .into_string()
        .map_err(std::env::VarError::NotUnicode)
}

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.filter(|value| !value.is_empty()).map(PathBuf::from)
}

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

/// `~/.rexafs` (created on demand, owner-only on Unix because it
/// holds `settings.json` with the Materials Project API key).
pub fn app_dir() -> Option<PathBuf> {
    let dir = home_dir()?.join(".rexafs");
    private_app_dir(&dir).ok()?;
    Some(dir)
}

/// Create private directories without a permissive-umask exposure window.
/// Existing caller-selected parents (for example /tmp) are never chmodded.
fn create_private_dirs(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn private_app_dir(path: &Path) -> std::io::Result<()> {
    create_private_dirs(path)?;
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other(
            "settings directory must be a real directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Use the open handle, and fail before reading/writing a credential if the
/// permissions cannot be secured. Also repairs older world-readable files.
fn private_file(file: &std::fs::File) -> std::io::Result<()> {
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("settings must be a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// `~/.rexafs/settings.json`, or the file named by `REXAFS_SETTINGS`
/// (scripted launches keep the user's real settings untouched).
fn settings_path() -> Option<PathBuf> {
    if let Some(p) = env_var_os("SETTINGS") {
        return Some(PathBuf::from(p));
    }
    app_dir().map(|d| d.join("settings.json"))
}

fn settings_read_path(current: &Path, explicit_override: bool) -> PathBuf {
    if !explicit_override && !current.exists() {
        if let Some(home) = current.parent().and_then(Path::parent) {
            return home.join(".xraytsubaki/settings.json");
        }
    }
    current.to_path_buf()
}

/// Default location for a downloaded AMCSD database.
pub fn default_amcsd_path() -> Option<PathBuf> {
    app_dir().map(|d| {
        let current = d.join("amcsd_cif2.db");
        let legacy = d.parent().unwrap_or(&d).join(".xraytsubaki/amcsd_cif2.db");
        if !current.exists() && legacy.exists() {
            legacy
        } else {
            current
        }
    })
}

impl UserSettings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|p| {
                // An explicit override or an existing new file is authoritative.
                Self::load_from(&settings_read_path(&p, env_var_os("SETTINGS").is_some())).ok()
            })
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        private_file(&file).map_err(|e| e.to_string())?;
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub fn save(&self) -> Result<(), String> {
        let path = settings_path().ok_or_else(|| "User home directory unavailable".to_string())?;
        self.save_to(&path)
    }

    /// Write the settings atomically with owner-only permissions (0600); the
    /// API key is stored in plain text, so the file must never be world-
    /// or group-readable. The key is never logged.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            create_private_dirs(dir).map_err(|e| e.to_string())?;
        }
        // Unique temp name + exclusive create: a pre-placed file or
        // symlink at the temp path makes the open fail instead of being
        // followed, so the key is never written through someone else's link.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = path.with_extension(format!("json.{}.{nonce:x}.tmp", std::process::id()));
        write_exclusive(&tmp, &text)?;
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
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
    if let Err(e) = private_file(&f)
        .and_then(|()| f.write_all(text.as_bytes()))
        .and_then(|()| f.sync_all())
    {
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
    #[cfg(unix)]
    fn private_settings_with_permissive_umask() {
        use std::os::unix::fs::PermissionsExt;
        if std::env::var_os("REXAFS_TEST_PERMISSIVE_UMASK").is_none() {
            let result = std::process::Command::new("sh")
                .args(["-c", "umask 000; exec \"$1\" --exact settings::tests::private_settings_with_permissive_umask --nocapture", "settings-test"])
                .arg(std::env::current_exe().unwrap())
                .env("REXAFS_TEST_PERMISSIVE_UMASK", "1")
                .output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            return;
        }
        let root =
            std::env::temp_dir().join(format!("rexafs-private-settings-{}", std::process::id()));
        let dir = root.join("config");
        let path = dir.join("settings.json");
        let settings = UserSettings {
            mp_api_key: "test-only-placeholder".into(),
            ..Default::default()
        };
        settings.save_to(&path).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&path), 0o600);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert_eq!(UserSettings::load_from(&path).unwrap(), settings);
        assert_eq!(
            mode(&path),
            0o600,
            "legacy settings must be repaired before reading"
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        settings.save_to(&path).unwrap();
        assert_eq!(mode(&path), 0o600, "replacement must remain private");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        private_app_dir(&dir).unwrap();
        assert_eq!(mode(&dir), 0o700);
        let link = root.join("redirected");
        std::os::unix::fs::symlink(&dir, &link).unwrap();
        assert!(private_app_dir(&link).is_err());
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "temporary credentials must be removed"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_settings_are_read_only_when_new_settings_are_absent() {
        let home = std::env::temp_dir().join(format!("rexafs-migration-{}", std::process::id()));
        let current = home.join(".rexafs/settings.json");
        let legacy = home.join(".xraytsubaki/settings.json");
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, r#"{"mp_api_key":"legacy"}"#).unwrap();
        assert_eq!(settings_read_path(&current, false), legacy);
        assert_eq!(settings_read_path(&current, true), current);
        UserSettings::default().save_to(&current).unwrap();
        assert_eq!(settings_read_path(&current, false), current);
        assert_eq!(
            UserSettings::load_from(&legacy).unwrap().mp_api_key,
            "legacy"
        );
        std::fs::remove_dir_all(home).unwrap();
    }

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
