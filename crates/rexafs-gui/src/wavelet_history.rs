//! Bounded immutable wavelet artifacts; the project carries small receipts only.
use crate::{analysis_store, group_identity::GroupId, params::PipelineParams};
use rexafs::WaveletMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WaveletArchive {
    pub entries: Vec<WaveletReceipt>,
    pub regions: Vec<WaveletSavedRegion>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct WaveletSavedRegion {
    pub map_digest: String,
    pub label: String,
    pub measurement: rexafs::WaveletRegionValue,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct WaveletReceipt {
    pub group: GroupId,
    pub label: String,
    pub settings: PipelineParams,
    pub definition: rexafs::Wavelet,
    pub path: PathBuf,
    pub digest: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct WaveletRecord {
    pub schema: u32,
    pub group: GroupId,
    pub label: String,
    pub settings: PipelineParams,
    pub map: WaveletMap,
    pub fourier: Option<rexafs::XrayFFTF>,
    pub fourier_error: Option<String>,
}
pub fn root() -> Result<PathBuf, String> {
    let parent = crate::settings::env_var_os("SETTINGS")
        .and_then(|p| PathBuf::from(p).parent().map(Path::to_path_buf))
        .or_else(crate::settings::app_dir)
        .ok_or("Application storage unavailable")?;
    Ok(parent.join("wavelet-results"))
}
pub fn retain(root: &Path, record: &WaveletRecord) -> Result<WaveletReceipt, String> {
    let (path, digest) = analysis_store::retain(root, record)?;
    Ok(WaveletReceipt {
        group: record.group.clone(),
        label: record.label.clone(),
        settings: record.settings.clone(),
        definition: record.map.settings().clone(),
        path,
        digest,
    })
}
pub fn read(receipt: &WaveletReceipt) -> Result<WaveletRecord, String> {
    let record: WaveletRecord = analysis_store::read(&receipt.path, &receipt.digest)?;
    if record.schema != 1
        || record.group != receipt.group
        || record.label != receipt.label
        || record.settings != receipt.settings
        || record.map.settings() != &receipt.definition
    {
        return Err("Saved wavelet identity mismatch".into());
    }
    Ok(record)
}
impl WaveletArchive {
    pub fn insert(&mut self, receipt: WaveletReceipt) {
        if !self
            .entries
            .iter()
            .any(|r| r.group == receipt.group && r.digest == receipt.digest)
        {
            self.entries.push(receipt);
        }
    }
    /// Coalesce this session's previews without removing loaded historical maps
    /// or maps referenced by historical region measurements.
    pub fn insert_preview(
        &mut self,
        receipt: WaveletReceipt,
        previews: &mut std::collections::HashMap<GroupId, String>,
    ) {
        let existed = self
            .entries
            .iter()
            .any(|r| r.group == receipt.group && r.digest == receipt.digest);
        let previous = previews.remove(&receipt.group);
        if let Some(digest) = &previous {
            self.entries.retain(|r| {
                r.group != receipt.group
                    || r.digest != *digest
                    || self
                        .regions
                        .iter()
                        .any(|region| region.map_digest == r.digest)
            });
        }
        if !existed || previous.as_deref() == Some(&receipt.digest) {
            previews.insert(receipt.group.clone(), receipt.digest.clone());
        }
        self.insert(receipt);
    }

    pub fn relocate(
        &mut self,
        f: &mut impl FnMut(&Path) -> Result<PathBuf, String>,
    ) -> Result<(), String> {
        for r in &mut self.entries {
            r.path = f(&r.path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_previews_keep_one_per_group_and_preserve_loaded_history() {
        let group = GroupId::new_result();
        let receipt = |digest: &str| WaveletReceipt {
            group: group.clone(),
            label: "Cu".into(),
            settings: Default::default(),
            definition: rexafs::Wavelet::new(2. ..=12.),
            path: PathBuf::from(digest),
            digest: digest.into(),
        };
        let mut archive = WaveletArchive::default();
        archive.insert(receipt("historical"));
        let mut previews = std::collections::HashMap::new();
        // A recalculation identical to a loaded map must not take ownership of it.
        archive.insert_preview(receipt("historical"), &mut previews);
        archive.insert_preview(receipt("first-preview"), &mut previews);
        archive.insert_preview(receipt("last-preview"), &mut previews);
        assert_eq!(
            archive
                .entries
                .iter()
                .map(|r| r.digest.as_str())
                .collect::<Vec<_>>(),
            ["historical", "last-preview"]
        );
        archive.insert_preview(receipt("historical"), &mut previews);
        archive.insert_preview(receipt("new-preview"), &mut previews);
        assert_eq!(
            archive
                .entries
                .iter()
                .map(|r| r.digest.as_str())
                .collect::<Vec<_>>(),
            ["historical", "new-preview"]
        );
    }

    #[test]
    fn embedded_maps_and_regions_survive_missing_cache_and_verify_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("maps");
        let k = (0..101).map(|i| i as f64 * 0.1).collect::<Vec<_>>();
        let chi = k
            .iter()
            .map(|v| (4. * v).sin() * (-((v - 5.) / 2.).powi(2)).exp())
            .collect::<Vec<_>>();
        let map = rexafs::Wavelet::new(1. ..=9.)
            .rstep(0.1)
            .rmax(4.)
            .calculate(&k, &chi)
            .unwrap();
        let measurement = map.integral(3. ..=7., 1. ..=3.).unwrap();
        let record = WaveletRecord {
            schema: 1,
            group: GroupId::new_result(),
            label: "Synthetic packet".into(),
            settings: Default::default(),
            map: map.clone(),
            fourier: None,
            fourier_error: None,
        };
        let receipt = retain(&root, &record).unwrap();
        let mut archive = WaveletArchive::default();
        archive.insert(receipt.clone());
        archive.insert(receipt.clone());
        assert_eq!(archive.entries.len(), 1);
        archive.regions.push(WaveletSavedRegion {
            map_digest: receipt.digest.clone(),
            label: receipt.label.clone(),
            measurement: measurement.clone(),
        });
        let project = crate::project::ProjectFile {
            wavelets: archive,
            ..Default::default()
        };
        let path = dir.path().join("wavelet.rxs");
        crate::project::save_with_storage(&path, &project, crate::project::DataStorage::Embedded)
            .unwrap();
        std::fs::remove_dir_all(root).unwrap();
        let restored =
            crate::project::load_with_cache_root(&path, || Ok(dir.path().join("restored")))
                .unwrap();
        assert!(restored.raw_files.is_empty());
        let receipt = &restored.wavelets.entries[0];
        assert_eq!(read(receipt).unwrap().map, map);
        assert_eq!(restored.wavelets.regions[0].measurement, measurement);
        let mut wrong = receipt.clone();
        wrong.definition.order += 1;
        assert!(read(&wrong).err().unwrap().contains("identity"));
        std::fs::write(&receipt.path, b"corrupted map").unwrap();
        assert!(read(receipt).err().unwrap().contains("checksum"));
    }

    /// A request beyond the measured k range is saved and restored as requested;
    /// the map reports the narrower interval it used separately.
    #[test]
    fn data_limited_maps_keep_the_requested_definition_in_history() {
        let dir = tempfile::tempdir().unwrap();
        let k = (0..181).map(|i| i as f64 * 0.05).collect::<Vec<_>>(); // to 9 Å⁻¹
        let chi = k.iter().map(|v| (4. * v).sin()).collect::<Vec<_>>();
        let requested = rexafs::Wavelet::new(2. ..=12.).rstep(0.1).rmax(4.);
        let map = requested.calculate(&k, &chi).unwrap();
        assert_eq!(map.k_support(), [2., 9.]);
        let record = WaveletRecord {
            schema: 1,
            group: GroupId::new_result(),
            label: "Short".into(),
            settings: Default::default(),
            map,
            fourier: None,
            fourier_error: None,
        };
        let receipt = retain(dir.path(), &record).unwrap();
        assert_eq!(receipt.definition, requested);
        assert_eq!(read(&receipt).unwrap().map.settings(), &requested);
    }
}
