use super::*;
use std::{cell::Cell, f64::consts::PI};
fn packet() -> (Vec<f64>, Vec<f64>) {
    let k: Vec<_> = (0..401).map(|i| i as f64 * 0.05).collect();
    let chi = k
        .iter()
        .map(|k| (-0.5 * ((k - 8.) / 1.2).powi(2)).exp() * (2. * 2.3 * k).sin())
        .collect();
    (k, chi)
}
#[test]
fn wavelet_sign_axes_and_amplitude_match_a_direct_dft() {
    let k: Vec<_> = (0..32).map(|i| i as f64 * 0.1).collect();
    let chi: Vec<_> = k
        .iter()
        .map(|v| (3.7 * v).sin() + 0.3 * (5.1 * v).cos())
        .collect();
    let radii = vec![0.7, 1.5, 2.7];
    let map = Wavelet::new(0. ..=3.1)
        .kstep(0.1)
        .kweight(0)
        .order(8)
        .nfft(64)
        .radii(radii.clone())
        .calculate(&k, &chi)
        .unwrap();
    // Independent O(N^2) sums, ordinary factorial/powers (not FFT or log kernel).
    let factorial = (1..=8).product::<usize>() as f64;
    for (row, &r) in radii.iter().enumerate() {
        for j in 0..k.len() {
            let mut re = 0.;
            let mut im = 0.;
            for l in 1..32 {
                let mut ur = 0.;
                let mut ui = 0.;
                for (n, &value) in chi.iter().enumerate() {
                    let phase = 2. * PI * l as f64 * n as f64 / 64.;
                    ur += value * phase.cos();
                    ui -= value * phase.sin();
                }
                let x = (8. / (2. * r)) * (2. * PI * l as f64 / (64. * 0.1));
                let kernel = 2. * PI / factorial * x.powi(8) * (-x).exp();
                let phase = 2. * PI * l as f64 * j as f64 / 64.;
                re += kernel * (ur * phase.cos() - ui * phase.sin()) / 64.;
                im += kernel * (ur * phase.sin() + ui * phase.cos()) / 64.;
            }
            assert!((map.real()[row * k.len() + j] - re).abs() < 1e-12);
            assert!((map.imaginary()[row * k.len() + j] - im).abs() < 1e-12);
        }
    }
    assert_eq!(map.input_chi(), chi);
    assert_eq!(map.r(), radii);
}
#[test]
fn wavelet_packet_localizes_and_fixed_order_survives_r_extension() {
    let (k, chi) = packet();
    let settings = Wavelet::new(1. ..=18.).kweight(0).rstep(0.05).rmax(3.);
    let short = settings.calculate(&k, &chi).unwrap();
    let long = settings.clone().rmax(6.).calculate(&k, &chi).unwrap();
    assert_eq!(short.real(), &long.real()[..short.real().len()]);
    assert_eq!(
        short.imaginary(),
        &long.imaginary()[..short.imaginary().len()]
    );
    let magnitude = long.magnitude();
    let (index, _) = magnitude
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    assert!((long.r()[index / k.len()] - 2.3).abs() < 0.1);
    assert!((long.k()[index % k.len()] - 8.).abs() < 0.2);
    assert_eq!(short.size(), &settings.estimate(&k).unwrap());
    assert!(short.phase(0.1).unwrap().iter().any(Option::is_none));
    assert!(short.phase(f64::NAN).is_err());
}
#[test]
fn wavelet_irregular_support_padding_taper_and_cancellation_are_explicit() {
    let k = vec![1., 1.3, 1.8, 2.6, 3., 3.9, 4.];
    let chi: Vec<_> = k.iter().map(|v| 2. * v + 3.).collect();
    let model = Wavelet::new(1. ..=4.)
        .kstep(0.1)
        .kweight(2)
        .rstep(0.1)
        .rmax(2.)
        .taper(0.5);
    let map = model.calculate(&k, &chi).unwrap();
    for (i, &x) in map.k().iter().enumerate() {
        if x < 1. {
            assert!(!map.support()[i]);
            assert_eq!(map.prepared_chi()[i], 0.);
        } else {
            assert!((map.prepared_chi()[i] - (2. * x + 3.)).abs() < 1e-12);
        }
    }
    assert_eq!(map.window()[10], 0.);
    assert!((map.window()[15] - 1.).abs() < 1e-12);
    assert_eq!(map.window()[40], 0.);
    assert!(map.warnings().iter().any(|w| w.contains("resampled")));
    let calls = Cell::new(0);
    assert_eq!(
        model
            .calculate_with_cancel(&k, &chi, || {
                calls.set(calls.get() + 1);
                calls.get() > 5
            })
            .unwrap_err(),
        WaveletError::Cancelled
    );
}
#[test]
fn wavelet_region_integral_uses_the_bilinear_scientific_surface() {
    let (k, chi) = packet();
    let mut map = Wavelet::new(1. ..=18.)
        .rstep(0.1)
        .rmax(4.)
        .kweight(0)
        .calculate(&k, &chi)
        .unwrap();
    // A positive affine/bilinear surface has an independent closed-form integral.
    for (j, &r) in map.data.r.iter().enumerate() {
        for (i, &k) in map.data.k.iter().enumerate() {
            map.data.real[j * map.data.k.len() + i] = 2. + k + 3. * r + 0.2 * k * r;
            map.data.imaginary[j * map.data.k.len() + i] = 0.;
        }
    }
    let [ka, kb] = [3.03, 8.17];
    let [ra, rb] = [0.33, 2.71];
    let expected = 2. * (kb - ka) * (rb - ra)
        + 0.5 * (kb * kb - ka * ka) * (rb - ra)
        + 1.5 * (rb * rb - ra * ra) * (kb - ka)
        + 0.05 * (kb * kb - ka * ka) * (rb * rb - ra * ra);
    let metric = map.integral(ka..=kb, ra..=rb).unwrap();
    assert!((metric.value - expected).abs() < 1e-10);
    let mean = map.mean(ka..=kb, ra..=rb).unwrap();
    assert!((mean.value - expected / ((kb - ka) * (rb - ra))).abs() < 1e-10);
    assert_eq!(mean.method, "bilinear_magnitude_mean_v1");
    let maximum = map.maximum(ka..=kb, ra..=rb).unwrap();
    assert!((maximum.value - (2. + kb + 3. * rb + 0.2 * kb * rb)).abs() < 1e-10);
    assert_eq!(maximum.k_range, [ka, kb]);
    assert_eq!(maximum.r_range, [ra, rb]);
    assert_eq!(maximum.method, "bilinear_magnitude_maximum_v1");
    assert!(map.mean(kb..=ka, ra..=rb).is_err());
    assert!(map.maximum(ka..=kb, rb..=ra).is_err());
    assert!(map.mean(ka..=kb, 0. ..=rb).is_err());
    assert!(map.maximum(0. ..=kb, ra..=rb).is_err());
    assert!(map.maximum(f64::NAN..=kb, ra..=rb).is_err());
    for (i, &k) in map.k().iter().enumerate() {
        assert!((map.slice_at_r(ra).unwrap()[i] - (2. + k + 3. * ra + 0.2 * k * ra)).abs() < 1e-10);
    }
    for (i, &r) in map.r().iter().enumerate() {
        assert!((map.slice_at_k(ka).unwrap()[i] - (2. + ka + 3. * r + 0.2 * ka * r)).abs() < 1e-10);
    }
    assert!(map.integral(0. ..=3., 1. ..=2.).is_err());
    assert!(map.integral(3. ..=8., 0. ..=2.).is_err());
    let json = serde_json::to_string(&map).unwrap();
    let restored: WaveletMap = serde_json::from_str(&json).unwrap();
    assert_eq!(map, restored);
    let mut bad: serde_json::Value = serde_json::from_str(&json).unwrap();
    bad["real"] = serde_json::json!([0.]);
    assert!(serde_json::from_value::<WaveletMap>(bad).is_err());
}
#[test]
fn wavelet_invalid_grids_fail_before_unbounded_allocation_or_truncation() {
    let (k, chi) = packet();
    for model in [
        Wavelet::new(25. ..=30.),
        Wavelet::new(8. ..=4.),
        Wavelet::new(1. ..=18.).kstep(1e-30),
        Wavelet::new(1. ..=18.).nfft(512),
        Wavelet::new(1. ..=18.).order(0),
        Wavelet::new(1. ..=18.).radii(vec![0., 1.]),
        Wavelet::new(1. ..=18.).rstep(1e-20),
        Wavelet::new(1. ..=18.).taper(10.),
        Wavelet::new(1. ..=18.).kweight(7),
    ] {
        assert!(model.calculate(&k, &chi).is_err());
    }
    let zero = Wavelet::new(1. ..=18.)
        .calculate(&k, &vec![0.; k.len()])
        .unwrap();
    assert!(zero.phase(0.).unwrap().iter().all(Option::is_none));
    assert!(zero.magnitude().iter().all(|v| *v == 0.));
}
#[test]
fn wavelet_k_range_beyond_the_data_is_limited_and_reported() {
    let (k, chi) = packet(); // measured k = 0–20 Å⁻¹
    let map = Wavelet::new(2. ..=30.).calculate(&k, &chi).unwrap();
    assert_eq!(map.settings().k_range, [2., 20.]);
    assert!(map
        .warnings()
        .iter()
        .any(|w| w.contains("limited to the measured data")));
    let inside = Wavelet::new(2. ..=12.).calculate(&k, &chi).unwrap();
    assert_eq!(inside.settings().k_range, [2., 12.]);
    assert!(!inside.warnings().iter().any(|w| w.contains("limited")));
    let error = Wavelet::new(25. ..=30.).calculate(&k, &chi).unwrap_err();
    assert!(error.to_string().contains("outside the measured k range"));
}
#[test]
fn wavelet_spectrum_api_prepares_on_a_copy_and_retains_settings() {
    let energy: Vec<_> = (0..401).map(|i| 8779. + 3. * i as f64).collect();
    let mu: Vec<_> = energy
        .iter()
        .map(|e| {
            let k = ((e - 8979.).max(0.) * 0.2625).sqrt();
            0.1 + 1. / (1. + (-(e - 8979.) / 1.5).exp())
                + if *e > 8989. {
                    0.1 * (4.6 * k).sin() / (1. + k)
                } else {
                    0.
                }
        })
        .collect();
    let mut spectrum = crate::Spectrum::from_arrays(&energy, &mu).unwrap();
    spectrum.set_e0(8979.);
    let original = spectrum.clone();
    let settings = Wavelet::new(2. ..=10.).rstep(0.1);
    let map = spectrum.wavelet(&settings).unwrap();
    assert_eq!(spectrum, original);
    assert!(map.preparation().unwrap().normalization.is_some());
    assert!(map.preparation().unwrap().background.is_some());
    let replay = map
        .settings()
        .calculate(map.input_k(), map.input_chi())
        .unwrap();
    assert_eq!(map.real(), replay.real());
    let corrected = spectrum
        .correct_fluorescence(
            &crate::FluorescenceCorrection::new("CuO", "Cu", "K")
                .line("Ka1")
                .angles(45., 45.),
        )
        .unwrap();
    assert!(corrected
        .wavelet(&settings)
        .unwrap_err()
        .to_string()
        .contains("XANES"));
}
