//! Inverse-transform checks against an explicit real inverse DFT.
use easyfft::{dyn_size::realfft::DynRealDft, num_complex::Complex};
use rexafs::xafs::{xafsutils::FTWindow, xrayfft::XrayFFTR};

fn spectrum() -> DynRealDft<f64> {
    let mut bins = vec![Complex::new(0.0, 0.0); 16];
    bins[1] = Complex::new(3.0, 4.0); // R = 2 Å: inside the window.
    bins[9] = Complex::new(100.0, 20.0); // R = 10 Å: must be removed.
    DynRealDft::new(7.0, &bins, 32) // DC must also be windowed.
}
fn run(transform: &mut XrayFFTR, input: &DynRealDft<f64>) -> Result<(), String> {
    // Only the first three displayed R values are supplied, but chi(R)
    // contains the full FFT. Display truncation must not affect filtering.
    #[cfg(not(feature = "ndarray-compat"))]
    let result = transform.xftr(&nalgebra::DVector::from_vec(vec![0., 1., 2.]), input);
    #[cfg(feature = "ndarray-compat")]
    let result = transform.xftr(ndarray::arr1(&[0., 1., 2.]).view(), input);
    result.map(|_| ()).map_err(|e| e.to_string())
}
#[test]
fn inverse_windows_all_bins_weights_the_correct_r_and_resizes_fft() {
    for nfft in [16, 32, 33, 64] {
        let kstep = std::f64::consts::PI / nfft as f64;
        for weight in [0., 2.] {
            let mut transform = XrayFFTR {
                nfft: Some(nfft),
                kstep: Some(kstep),
                rmin: Some(1.0),
                rmax: Some(4.0),
                dr: Some(0.2),
                rweight: Some(weight),
                window: Some(FTWindow::Hanning),
                qmax_out: Some(0.47),
                ..Default::default()
            };
            run(&mut transform, &spectrum()).unwrap();
            let q = transform.get_q().unwrap();
            let chiq = transform.get_chiq().unwrap();
            assert_eq!(q.len(), chiq.len());
            assert_eq!(q.len(), (0.47 / kstep).floor() as usize + 1);
            let scale = std::f64::consts::PI.sqrt() / kstep / nfft as f64;
            for (i, (&q, &actual)) in q.iter().zip(chiq.iter()).enumerate() {
                assert!(
                    (q - i as f64 * kstep).abs() < 1e-14,
                    "q is an FFT grid, not a stretched linspace"
                );
                let phase = 2.0 * std::f64::consts::PI * 2.0 * i as f64 / nfft as f64;
                let expected =
                    scale * 2.0_f64.powf(weight) * (6.0 * phase.cos() - 8.0 * phase.sin());
                assert!(
                    (actual - expected).abs() < 1e-11,
                    "nfft={nfft}, weight={weight}, i={i}: {actual} vs {expected}"
                );
            }
        }
    }
}
#[test]
fn inverse_output_limit_cannot_exceed_available_samples() {
    let mut transform = XrayFFTR {
        nfft: Some(32),
        qmax_out: Some(1e100),
        ..Default::default()
    };
    run(&mut transform, &spectrum()).unwrap();
    assert_eq!(transform.get_q().unwrap().len(), 32);
    assert_eq!(transform.get_chiq().unwrap().len(), 32);
}
#[test]
fn inverse_rejects_invalid_settings_before_fft_or_allocation() {
    for transform in [
        XrayFFTR {
            nfft: Some(0),
            ..Default::default()
        },
        XrayFFTR {
            kstep: Some(0.),
            ..Default::default()
        },
        XrayFFTR {
            kstep: Some(f64::NAN),
            ..Default::default()
        },
        XrayFFTR {
            rweight: Some(-1.),
            ..Default::default()
        },
        XrayFFTR {
            qmax_out: Some(-1.),
            ..Default::default()
        },
        XrayFFTR {
            rmin: Some(3.),
            rmax: Some(1.),
            ..Default::default()
        },
    ] {
        assert!(run(&mut transform.clone(), &spectrum()).is_err());
    }
}
