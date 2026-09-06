//! Official desktop release discovery and checksum-verified downloads.
//! Installation stays an explicit user action; no running app or project is replaced.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};

const REPOSITORY: &str = "https://github.com/Ameyanagi/rexafs";
const API: &str = "https://api.github.com/repos/Ameyanagi/rexafs/releases?per_page=100";
const STABLE_API: &str = "https://api.github.com/repos/Ameyanagi/rexafs/releases/latest";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Nightly,
}
impl UpdateChannel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stable => "Stable",
            Self::Nightly => "Nightly",
        }
    }
}

pub fn build_info() -> serde_json::Value {
    serde_json::json!({"version":env!("CARGO_PKG_VERSION"), "channel":installed_channel(),
        "release_tag":installed_tag(), "commit":option_env!("GITHUB_SHA").unwrap_or("development"),
        "built_at":option_env!("REXAFS_BUILD_UTC")})
}
pub fn installed_channel() -> UpdateChannel {
    if option_env!("REXAFS_BUILD_CHANNEL") == Some("nightly") {
        UpdateChannel::Nightly
    } else {
        UpdateChannel::Stable
    }
}
pub fn installed_tag() -> &'static str {
    option_env!("REXAFS_BUILD_TAG").unwrap_or(concat!("v", env!("CARGO_PKG_VERSION")))
}
pub fn installed_label() -> String {
    match installed_channel() {
        UpdateChannel::Stable => env!("CARGO_PKG_VERSION").into(),
        UpdateChannel::Nightly => format!("{} · {}", env!("CARGO_PKG_VERSION"), installed_tag()),
    }
}
pub fn application_name() -> &'static str {
    if installed_channel() == UpdateChannel::Nightly {
        "rexafs Nightly"
    } else {
        "rexafs"
    }
}
fn desktop_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        // Linux/Windows archives have not yet passed public graphical qualification.
        _ => None,
    }
}

#[derive(Clone, Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    html_url: String,
    assets: Vec<Asset>,
}
#[derive(Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    digest: Option<String>,
    browser_download_url: String,
}
#[derive(Clone)]
pub struct AvailableRelease {
    pub tag: String,
    pub url: String,
    pub asset: Option<Asset>,
}
#[derive(Clone)]
pub struct UpdateCheck {
    pub release: Option<AvailableRelease>,
    pub available: bool,
}

fn stable_version(tag: &str) -> Option<semver::Version> {
    let v = semver::Version::parse(tag.strip_prefix('v')?).ok()?;
    (v.pre.is_empty() && v.build.is_empty()).then_some(v)
}
fn nightly_order(tag: &str) -> Option<(&str, u64)> {
    let (date, run) = tag.strip_prefix("nightly-")?.split_once('-')?;
    if date.len() != 8
        || chrono::NaiveDate::parse_from_str(date, "%Y%m%d").is_err()
        || run.is_empty()
        || !run.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some((date, run.parse().ok()?))
}
fn checksum(asset: &Asset) -> Result<&str, String> {
    asset
        .digest
        .as_deref()
        .and_then(|s| s.strip_prefix("sha256:"))
        .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| "GitHub did not provide a valid SHA-256 digest for this asset.".into())
}
fn select_release(
    releases: Vec<Release>,
    channel: UpdateChannel,
    current_channel: UpdateChannel,
    current_tag: &str,
    current_version: &str,
    built_at: Option<&str>,
    target: Option<&str>,
) -> UpdateCheck {
    let release = releases
        .into_iter()
        .filter(|r| {
            !r.draft
                && r.published_at.is_some()
                && r.html_url == format!("{REPOSITORY}/releases/tag/{}", r.tag_name)
                && match channel {
                    UpdateChannel::Stable => !r.prerelease && stable_version(&r.tag_name).is_some(),
                    UpdateChannel::Nightly => r.prerelease && nightly_order(&r.tag_name).is_some(),
                }
        })
        .max_by(|a, b| match channel {
            UpdateChannel::Stable => stable_version(&a.tag_name).cmp(&stable_version(&b.tag_name)),
            UpdateChannel::Nightly => nightly_order(&a.tag_name).cmp(&nightly_order(&b.tag_name)),
        });
    let Some(r) = release else {
        return UpdateCheck {
            release: None,
            available: false,
        };
    };
    let available = if channel != current_channel {
        true
    } else {
        match channel {
            UpdateChannel::Stable => {
                stable_version(&r.tag_name) > semver::Version::parse(current_version).ok()
            }
            UpdateChannel::Nightly => {
                if let Some(current) = nightly_order(current_tag) {
                    nightly_order(&r.tag_name) > Some(current)
                } else {
                    r.tag_name != current_tag
                        && built_at.is_none_or(|t| r.published_at.as_deref().is_some_and(|p| p > t))
                }
            }
        }
    };
    let asset = target.and_then(|target| {
        r.assets.into_iter().find(|a| {
            a.name.starts_with("rexafs-")
                && a.name.ends_with(&format!("-{target}.zip"))
                && a.name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                && a.size > 0
                && a.size <= 1024 * 1024 * 1024
                && checksum(a).is_ok()
                && a.browser_download_url
                    == format!("{REPOSITORY}/releases/download/{}/{}", r.tag_name, a.name)
        })
    });
    UpdateCheck {
        release: Some(AvailableRelease {
            tag: r.tag_name,
            url: r.html_url,
            asset,
        }),
        available,
    }
}
fn agent(seconds: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(seconds)))
        .build()
        .into()
}
pub fn check(channel: UpdateChannel) -> Result<UpdateCheck, String> {
    let mut response = agent(15)
        .get(if channel == UpdateChannel::Stable {
            STABLE_API
        } else {
            API
        })
        .header("User-Agent", concat!("rexafs/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Could not check GitHub releases: {e}"))?;
    // A stable release must remain discoverable after hundreds of nightlies.
    let releases = if channel == UpdateChannel::Stable {
        response.body_mut().read_json::<Release>().map(|r| vec![r])
    } else {
        response.body_mut().read_json::<Vec<Release>>()
    }
    .map_err(|e| format!("Invalid GitHub release response: {e}"))?;
    Ok(select_release(
        releases,
        channel,
        installed_channel(),
        installed_tag(),
        env!("CARGO_PKG_VERSION"),
        option_env!("REXAFS_BUILD_UTC"),
        desktop_target(),
    ))
}

fn verify_stream(
    mut reader: impl Read,
    mut output: impl Write,
    expected_size: u64,
    expected_hash: &str,
) -> Result<(), String> {
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > expected_size {
            return Err("Download exceeds the published asset size.".into());
        }
        hash.update(&buffer[..n]);
        output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
    }
    if total != expected_size || !hex(&hash.finalize()).eq_ignore_ascii_case(expected_hash) {
        return Err("Download failed SHA-256/size verification. No installer was opened.".into());
    }
    Ok(())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn download(release: &AvailableRelease) -> Result<PathBuf, String> {
    let asset = release
        .asset
        .as_ref()
        .ok_or("No verified desktop asset is available for this platform.")?;
    let digest = checksum(asset)?;
    let root = crate::settings::app_dir()
        .ok_or("User update directory is unavailable")?
        .join("updates")
        .join(&release.tag);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let destination = root.join(&asset.name);
    if destination.exists() {
        verify_stream(
            std::fs::File::open(&destination).map_err(|e| e.to_string())?,
            std::io::sink(),
            asset.size,
            digest,
        )
        .map_err(|error| {
            format!(
                "{error} Remove the damaged cached download at {} and try again.",
                destination.display()
            )
        })?;
        return Ok(destination);
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = root.join(format!(".download-{}-{nonce}.part", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut response = agent(180)
            .get(&asset.browser_download_url)
            .header("User-Agent", "rexafs-updater")
            .call()
            .map_err(|e| e.to_string())?;
        verify_stream(
            response.body_mut().as_reader(),
            &mut file,
            asset.size,
            digest,
        )?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        // Publish the completed file without replacing an existing download.
        std::fs::hard_link(&temporary, &destination).map_err(|e| e.to_string())?;
        Ok(destination)
    })();
    let _ = std::fs::remove_file(temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str, pre: bool, date: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: pre,
            published_at: Some(date.into()),
            html_url: format!("{REPOSITORY}/releases/tag/{tag}"),
            assets: vec![],
        }
    }
    #[test]
    fn channels_do_not_cross_and_stable_orders_by_version() {
        let releases = vec![
            release("v0.1.2", false, "2026-09-05T00:00:00Z"),
            release("v0.1.1", false, "2026-09-07T00:00:00Z"),
            release("v0.2.0-rc.1", true, "2026-09-07T01:00:00Z"),
            release("nightly-20260907-123", true, "2026-09-07T02:00:00Z"),
            // A delayed rerun must not promote an older nightly over a newer source.
            release("nightly-20260906-122", true, "2026-09-08T02:00:00Z"),
        ];
        let s = select_release(
            releases.clone(),
            UpdateChannel::Stable,
            UpdateChannel::Stable,
            "v0.1.1",
            "0.1.1",
            None,
            None,
        );
        assert!(s.available);
        assert_eq!(s.release.unwrap().tag, "v0.1.2");
        let n = select_release(
            releases.clone(),
            UpdateChannel::Nightly,
            UpdateChannel::Stable,
            "v0.1.1",
            "0.1.1",
            None,
            None,
        );
        assert!(n.available);
        assert_eq!(n.release.unwrap().tag, "nightly-20260907-123");
        let same = select_release(
            releases.clone(),
            UpdateChannel::Nightly,
            UpdateChannel::Nightly,
            "nightly-20260907-123",
            "0.1.1",
            None,
            None,
        );
        assert!(!same.available);
        let newer = select_release(
            releases.clone(),
            UpdateChannel::Stable,
            UpdateChannel::Stable,
            "v0.2.0",
            "0.2.0",
            None,
            None,
        );
        assert!(!newer.available);
        let switch = select_release(
            releases,
            UpdateChannel::Stable,
            UpdateChannel::Nightly,
            "nightly-20260907-123",
            "0.2.0",
            None,
            None,
        );
        assert!(
            switch.available,
            "choosing Stable explicitly permits returning from a nightly"
        );
    }
    #[test]
    fn only_official_matching_assets_with_checksums_are_downloadable() {
        let mut r = release("v0.1.2", false, "2026-09-07T00:00:00Z");
        let name = "rexafs-0.1.2-aarch64-apple-darwin.zip";
        let good = Asset {
            name: name.into(),
            size: 100,
            digest: Some(format!("sha256:{}", "a".repeat(64))),
            browser_download_url: format!("{REPOSITORY}/releases/download/v0.1.2/{name}"),
        };
        for bad in [
            Asset {
                digest: None,
                ..good.clone()
            },
            Asset {
                browser_download_url: "https://example.com/installer.zip".into(),
                ..good.clone()
            },
            Asset {
                size: 0,
                ..good.clone()
            },
        ] {
            r.assets = vec![bad];
            let result = select_release(
                vec![r.clone()],
                UpdateChannel::Stable,
                UpdateChannel::Stable,
                "v0.1.1",
                "0.1.1",
                None,
                Some("aarch64-apple-darwin"),
            );
            assert!(result.release.unwrap().asset.is_none());
        }
        r.assets = vec![good];
        assert!(
            select_release(
                vec![r],
                UpdateChannel::Stable,
                UpdateChannel::Stable,
                "v0.1.1",
                "0.1.1",
                None,
                Some("aarch64-apple-darwin")
            )
            .release
            .unwrap()
            .asset
            .is_some()
        );
    }
    #[test]
    fn partial_corrupt_and_oversized_downloads_are_rejected() {
        let bytes = b"known release artifact";
        let hash = hex(&Sha256::digest(bytes));
        assert!(verify_stream(&bytes[..], std::io::sink(), bytes.len() as u64, &hash).is_ok());
        for (size, digest) in [
            (1, hash.clone()),
            (100, hash.clone()),
            (bytes.len() as u64, "0".repeat(64)),
        ] {
            assert!(verify_stream(&bytes[..], std::io::sink(), size, &digest).is_err());
        }
    }
}
