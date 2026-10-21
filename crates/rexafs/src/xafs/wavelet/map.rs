use super::*;
use crate::xafs::{background::BackgroundMethod, normalization::NormalizationMethod};

/// Provenance for a spectrum-prepared map. The retained k/χ arrays are its direct
/// replay inputs; these settings explain their preparation without duplicating
/// all normalization/background output arrays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveletPreparation {
    /// Rexafs package version that prepared this map.
    pub software_version: String,
    /// Selected array backend; historical numerical policies can differ.
    pub backend: String,
    /// Measured E₀ (eV) and normalization edge step, if available.
    pub e0: Option<f64>,
    /// Fitted edge step in original absorption units, if available.
    pub edge_step: Option<f64>,
    /// Resolved normalization settings, without large derived arrays.
    pub normalization: Option<NormalizationMethod>,
    /// Resolved AUTOBK settings, without large derived arrays.
    pub background: Option<BackgroundMethod>,
}
impl WaveletPreparation {
    pub(super) fn from_spectrum(source: &crate::Spectrum) -> Self {
        let mut settings = crate::Spectrum::new();
        settings.normalization = source.normalization.clone();
        settings.background = source.background.clone();
        if let Some(NormalizationMethod::MBack(m)) = &mut settings.normalization {
            if let Some(result) = &m.result {
                m.options.pre_edge = Some(result.pre_edge);
                m.options.post_edge = Some(result.post_edge);
                m.e0 = Some(result.e0);
            }
        }
        settings.invalidate_derived();
        Self {
            software_version: env!("CARGO_PKG_VERSION").into(),
            backend: if cfg!(feature = "ndarray-compat") {
                "ndarray-compat"
            } else {
                "nalgebra"
            }
            .into(),
            e0: source.e0(),
            edge_step: source
                .normalization
                .as_ref()
                .and_then(|n| n.get_edge_step()),
            normalization: settings.normalization,
            background: settings.background,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MapData {
    pub method: String,
    pub settings: Wavelet,
    pub size: WaveletSize,
    pub input_k: Vec<f64>,
    pub input_chi: Vec<f64>,
    pub k: Vec<f64>,
    pub r: Vec<f64>,
    pub prepared_chi: Vec<f64>,
    pub window: Vec<f64>,
    pub support: Vec<bool>,
    pub real: Vec<f64>,
    pub imaginary: Vec<f64>,
    pub preparation: Option<WaveletPreparation>,
    pub warnings: Vec<String>,
}

/// Owned Cauchy map with immutable arrays and validated deserialization.
/// Flattened complex arrays use **rows=R, columns=k**: index `row*k.len()+column`.
/// Magnitude/phase/slices are derived on demand; display colors/crops never alter
/// the scientific arrays. Its numerical units are those of k^weight χ under the
/// discrete-filter convention, not those of the ordinary XAFS Fourier transform.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MapData")]
pub struct WaveletMap {
    #[serde(flatten)]
    pub(super) data: MapData,
}
impl TryFrom<MapData> for WaveletMap {
    type Error = WaveletError;
    fn try_from(data: MapData) -> Result<Self> {
        let expected = calculate::layout(&data.settings, &data.input_k)?;
        if data.method != "cauchy_v1"
            || data.k != expected.k
            || data.r != expected.r
            || data.size != expected.size
            || data.input_chi.len() != data.input_k.len()
            || data.real.len() != data.size.cells
            || data.imaginary.len() != data.size.cells
            || data.prepared_chi.len() != data.k.len()
            || data.window.len() != data.k.len()
            || data.support.len() != data.k.len()
        {
            return Err(invalid(
                "stored map dimensions, grid or method do not match its definition",
            ));
        }
        if data
            .input_chi
            .iter()
            .chain(&data.real)
            .chain(&data.imaginary)
            .chain(&data.prepared_chi)
            .chain(&data.window)
            .any(|v| !v.is_finite())
        {
            return Err(invalid("stored map contains nonfinite numerical data"));
        }
        if data
            .real
            .iter()
            .zip(&data.imaginary)
            .any(|(a, b)| !a.hypot(*b).is_finite())
        {
            return Err(invalid("stored complex magnitude overflowed"));
        }
        Ok(Self { data })
    }
}

/// Region statistic of the bilinear surface through native magnitude samples.
/// k units (Å⁻¹) times R units (Å) cancel; result units equal those of k^weight χ.
/// No display texture, phase integral or error estimate participates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveletRegionValue {
    /// Finite, nonnegative magnitude statistic; `method` identifies the operation.
    pub value: f64,
    /// Exact requested k bounds (Å⁻¹).
    pub k_range: [f64; 2],
    /// Exact requested R bounds (Å).
    pub r_range: [f64; 2],
    /// Explicit units, including selected k weight.
    pub unit: String,
    /// Numerical convention: `bilinear_magnitude_v1` for an integral, or
    /// `bilinear_magnitude_mean_v1` / `bilinear_magnitude_maximum_v1`.
    pub method: String,
}
impl WaveletMap {
    /// Named numerical transform convention.
    pub fn method(&self) -> &str {
        &self.data.method
    }
    /// Borrow the complete requested transform definition, including explicit grids.
    pub fn settings(&self) -> &Wavelet {
        &self.data.settings
    }
    /// k interval actually transformed, in Å⁻¹ (since 0.2.15): the requested
    /// `settings().k_range` limited to the measured input k range. It equals the
    /// requested range when that lies inside the data; region statistics must
    /// lie inside it. The requested definition itself is never rewritten.
    pub fn k_support(&self) -> [f64; 2] {
        let k = &self.data.input_k;
        let [lo, hi] = self.data.settings.k_range;
        [lo.max(k[0]), hi.min(k[k.len() - 1])]
    }
    /// Validated output dimensions and estimated buffer storage.
    pub fn size(&self) -> &WaveletSize {
        &self.data.size
    }
    /// Original measured k coordinates (Å⁻¹), before any linear resampling.
    pub fn input_k(&self) -> &[f64] {
        &self.data.input_k
    }
    /// Original dimensionless, unweighted χ, unchanged.
    pub fn input_chi(&self) -> &[f64] {
        &self.data.input_chi
    }
    /// Zero-origin uniform k columns (Å⁻¹), including labeled padding.
    pub fn k(&self) -> &[f64] {
        &self.data.k
    }
    /// Strictly positive R rows (Å), not phase-corrected bond lengths.
    pub fn r(&self) -> &[f64] {
        &self.data.r
    }
    /// Resampled unweighted χ, zero outside the selected measured support.
    pub fn prepared_chi(&self) -> &[f64] {
        &self.data.prepared_chi
    }
    /// Support/taper multipliers applied before weighting; zero outside support.
    pub fn window(&self) -> &[f64] {
        &self.data.window
    }
    /// True where a k sample lies in selected measured support; false is padding.
    pub fn support(&self) -> &[bool] {
        &self.data.support
    }
    /// Flattened real values, rows=R and columns=k, in units of k^weight χ.
    pub fn real(&self) -> &[f64] {
        &self.data.real
    }
    /// Flattened imaginary values with the same axes/orientation as real.
    pub fn imaginary(&self) -> &[f64] {
        &self.data.imaginary
    }
    /// Spectrum preparation settings when the high-level spectrum API was used.
    /// None for a direct k/χ array calculation; those arrays are still fully retained.
    pub fn preparation(&self) -> Option<&WaveletPreparation> {
        self.data.preparation.as_ref()
    }
    /// Interpretation and boundary/grid diagnostics; no confidence claim is implied.
    pub fn warnings(&self) -> &[String] {
        &self.data.warnings
    }
    /// Independent magnitude array, rows=R and columns=k; no peak-height scaling.
    pub fn magnitude(&self) -> Vec<f64> {
        self.real()
            .iter()
            .zip(self.imaginary())
            .map(|(a, b)| a.hypot(*b))
            .collect()
    }
    /// Phase in radians, masked below `relative_floor` times the map's maximum
    /// magnitude. Fraction must lie in `[0, 1]`. Zero-amplitude cells are always None.
    /// This display mask does not modify data and does not create a phase metric.
    pub fn phase(&self, relative_floor: f64) -> Result<Vec<Option<f64>>> {
        if !relative_floor.is_finite() || !(0. ..=1.).contains(&relative_floor) {
            return Err(invalid(
                "phase magnitude floor must be a fraction from 0 to 1",
            ));
        }
        let max = self
            .real()
            .iter()
            .zip(self.imaginary())
            .map(|(a, b)| a.hypot(*b))
            .fold(0., f64::max);
        Ok(self
            .real()
            .iter()
            .zip(self.imaginary())
            .map(|(re, im)| {
                let magnitude = re.hypot(*im);
                (magnitude > 0. && magnitude >= max * relative_floor).then(|| im.atan2(*re))
            })
            .collect())
    }
    /// Interpolated magnitude versus k at one covered R (Å), on every k column.
    pub fn slice_at_r(&self, radius: f64) -> Result<Vec<f64>> {
        let (a, b, t) = bracket(self.r(), radius)?;
        Ok((0..self.k().len())
            .map(|i| self.magnitude_cell(a, i) * (1. - t) + self.magnitude_cell(b, i) * t)
            .collect())
    }
    /// Interpolated magnitude versus R at one covered k (Å⁻¹), on every R row.
    pub fn slice_at_k(&self, k: f64) -> Result<Vec<f64>> {
        let (a, b, t) = bracket(self.k(), k)?;
        Ok((0..self.r().len())
            .map(|i| self.magnitude_cell(i, a) * (1. - t) + self.magnitude_cell(i, b) * t)
            .collect())
    }
    fn magnitude_cell(&self, row: usize, column: usize) -> f64 {
        let i = row * self.k().len() + column;
        self.real()[i].hypot(self.imaginary()[i])
    }
    fn sample(&self, k: f64, r: f64) -> Result<f64> {
        let (ka, kb, kt) = bracket(self.k(), k)?;
        let (ra, rb, rt) = bracket(self.r(), r)?;
        let a = self.magnitude_cell(ra, ka) * (1. - kt) + self.magnitude_cell(ra, kb) * kt;
        let b = self.magnitude_cell(rb, ka) * (1. - kt) + self.magnitude_cell(rb, kb) * kt;
        Ok(a * (1. - rt) + b * rt)
    }
    /// Integrate native |W| over a completely covered rectangle, using its bilinear
    /// surface and exact clipped-cell trapezoids. k is Å⁻¹ and R is Å. Both ranges
    /// must have positive width; k must also lie inside selected measured support.
    /// Display cropping, color choice and image downsampling have no effect.
    pub fn integral(
        &self,
        k: RangeInclusive<f64>,
        r: RangeInclusive<f64>,
    ) -> Result<WaveletRegionValue> {
        let kr = [*k.start(), *k.end()];
        let rr = [*r.start(), *r.end()];
        if kr[0] < self.k_support()[0] || kr[1] > self.k_support()[1] {
            return Err(invalid(
                "wavelet integral must lie inside selected measured k support",
            ));
        }
        let ks = integration_grid(self.k(), kr)?;
        let rs = integration_grid(self.r(), rr)?;
        let mut value = 0.;
        for rw in rs.windows(2) {
            for kw in ks.windows(2) {
                value += 0.25
                    * (kw[1] - kw[0])
                    * (rw[1] - rw[0])
                    * (self.sample(kw[0], rw[0])?
                        + self.sample(kw[1], rw[0])?
                        + self.sample(kw[0], rw[1])?
                        + self.sample(kw[1], rw[1])?);
            }
        }
        if !value.is_finite() {
            return Err(invalid("wavelet integral overflowed"));
        }
        Ok(WaveletRegionValue {
            value,
            k_range: kr,
            r_range: rr,
            unit: if self.settings().kweight == 0 {
                "dimensionless".into()
            } else {
                format!("Å^-{}", self.settings().kweight)
            },
            method: "bilinear_magnitude_v1".into(),
        })
    }

    /// Area-weighted mean of native |W| in a k–R rectangle (since 0.2.10).
    /// Divides [`Self::integral`] by Δk ΔR, rather than averaging grid cells;
    /// therefore nonuniform R sampling does not bias the mean. k is Å⁻¹ and R
    /// is Å. Both ranges must be finite, increasing and completely covered.
    /// Units are those of k^weight χ; display sampling and colors have no effect.
    pub fn mean(
        &self,
        k: RangeInclusive<f64>,
        r: RangeInclusive<f64>,
    ) -> Result<WaveletRegionValue> {
        let area = (k.end() - k.start()) * (r.end() - r.start());
        let mut result = self.integral(k, r)?;
        result.value /= area;
        if !result.value.is_finite() {
            return Err(invalid("wavelet mean overflowed"));
        }
        result.method = "bilinear_magnitude_mean_v1".into();
        Ok(result)
    }

    /// Maximum of the bilinear native |W| surface in a k–R rectangle (since 0.2.10).
    /// Includes interpolated rectangle edges and corners, not just enclosed grid
    /// samples. A bilinear cell reaches its maximum at a corner. k is Å⁻¹ and R
    /// is Å; ranges must be finite, increasing and fully covered, including the
    /// selected measured k support. Units are those of k^weight χ. No peak
    /// interpolation beyond the bilinear surface or uncertainty is inferred.
    pub fn maximum(
        &self,
        k: RangeInclusive<f64>,
        r: RangeInclusive<f64>,
    ) -> Result<WaveletRegionValue> {
        let kr = [*k.start(), *k.end()];
        let rr = [*r.start(), *r.end()];
        if kr[0] < self.k_support()[0] || kr[1] > self.k_support()[1] {
            return Err(invalid(
                "wavelet region must lie inside selected measured k support",
            ));
        }
        let ks = integration_grid(self.k(), kr)?;
        let rs = integration_grid(self.r(), rr)?;
        let mut value: f64 = 0.;
        for r in rs {
            for &k in &ks {
                value = value.max(self.sample(k, r)?);
            }
        }
        Ok(WaveletRegionValue {
            value,
            k_range: kr,
            r_range: rr,
            unit: if self.settings().kweight == 0 {
                "dimensionless".into()
            } else {
                format!("Å^-{}", self.settings().kweight)
            },
            method: "bilinear_magnitude_maximum_v1".into(),
        })
    }
}
fn bracket(axis: &[f64], x: f64) -> Result<(usize, usize, f64)> {
    if !x.is_finite() || x < axis[0] || x > axis[axis.len() - 1] {
        return Err(invalid(
            "requested coordinate is outside the retained scientific map",
        ));
    }
    let b = axis.partition_point(|v| *v < x);
    if b == 0 {
        return Ok((0, 0, 0.));
    }
    Ok((b - 1, b, (x - axis[b - 1]) / (axis[b] - axis[b - 1])))
}
fn integration_grid(axis: &[f64], range: [f64; 2]) -> Result<Vec<f64>> {
    if !range[0].is_finite() || !range[1].is_finite() || range[0] >= range[1] {
        return Err(invalid("integral ranges must be finite and increasing"));
    }
    bracket(axis, range[0])?;
    bracket(axis, range[1])?;
    let mut grid = vec![range[0]];
    grid.extend(
        axis.iter()
            .copied()
            .filter(|v| *v > range[0] && *v < range[1]),
    );
    grid.push(range[1]);
    Ok(grid)
}
