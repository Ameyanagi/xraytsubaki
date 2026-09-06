use rexafs::{io, BackgroundMethod, NormalizationMethod, PrePostEdge, Spectrum, XrayFFTF, AUTOBK};

fn sample() -> Spectrum {
    io::read_qas_transmission(format!(
        "{}/tests/testfiles/Ru_QAS.dat",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn terminal_stage_matches_explicit_stages() {
    let mut automatic = sample();
    automatic.fft().unwrap();
    let mut explicit = sample();
    explicit
        .find_e0()
        .unwrap()
        .normalize()
        .unwrap()
        .calc_background()
        .unwrap()
        .fft()
        .unwrap();
    {
        let spectrum = &explicit;
        assert_eq!(automatic.e0(), spectrum.e0());
        assert_eq!(automatic.k(), spectrum.k());
        assert_eq!(automatic.chi(), spectrum.chi());
        assert_eq!(automatic.r(), spectrum.r());
        assert_eq!(automatic.chir_mag(), spectrum.chir_mag());
    }
    let mag = automatic.chir_mag().unwrap();
    let re = automatic.chir_real().unwrap();
    let im = automatic.chir_imag().unwrap();
    for ((mag, re), im) in mag.iter().zip(re.iter()).zip(im.iter()) {
        assert!((mag - re.hypot(*im)).abs() < 1e-10);
    }
}

#[test]
fn invalid_arrays_return_errors_before_processing() {
    for (energy, mu) in [
        (vec![], vec![]),
        (vec![2., 1., 3.], vec![1.]),
        (vec![1., 2., 2.], vec![1., 2., 3.]),
        (vec![1., f64::NAN, 3.], vec![1., 2., 3.]),
        (vec![1., 2., 3.], vec![1., f64::INFINITY, 3.]),
    ] {
        assert!(Spectrum::from_arrays(&energy, &mu).is_err());
    }
}

#[test]
fn edge_changes_invalidate_results_and_are_used() {
    let mut spectrum = sample();
    spectrum.fft().unwrap();
    let edge = spectrum.e0().unwrap() + 0.25;
    spectrum.set_e0(edge);
    assert!(spectrum.norm().is_none());
    assert!(spectrum.chi().is_none());
    assert!(spectrum.r().is_none());
    spectrum.fft().unwrap();
    assert_eq!(spectrum.e0(), Some(edge));
    if let Some(BackgroundMethod::AUTOBK(a)) = &spectrum.background {
        assert_eq!(a.ek0, Some(edge));
    }
    for e0 in [f64::NAN, f64::INFINITY, 0., 1e9] {
        spectrum.set_e0(e0);
        assert!(spectrum.fft().is_err());
        assert!(spectrum.r().is_none());
    }
}

#[test]
fn stage_configuration_is_respected_and_only_dependents_are_invalidated() {
    let mut spectrum = sample();
    let mut norm = PrePostEdge::new();
    norm.pre_edge_start = Some(-200.);
    norm.pre_edge_end = Some(-65.);
    spectrum
        .set_normalization_method(Some(NormalizationMethod::PrePostEdge(norm)))
        .unwrap();
    let mut background = AUTOBK::new();
    background.rbkg = Some(1.2);
    spectrum
        .set_background_method(Some(BackgroundMethod::AUTOBK(background)))
        .unwrap();
    let mut transform = XrayFFTF::new();
    transform.kweight = Some(1.);
    spectrum.set_fft(transform.clone()).fft().unwrap();
    let norm = spectrum.norm().unwrap();
    let chi_address = spectrum.chi().unwrap().as_ptr();
    let chi = spectrum.chi().unwrap().to_vec();
    let old_r = spectrum.chir_mag().unwrap();
    transform.kweight = Some(3.);
    spectrum.set_fft(transform);
    assert!(spectrum.r().is_none());
    spectrum.fft().unwrap();
    assert_eq!(spectrum.chi().unwrap().as_ptr(), chi_address);
    assert_eq!(spectrum.chi().unwrap(), chi);
    assert_eq!(spectrum.norm().unwrap(), norm);
    assert_ne!(spectrum.chir_mag().unwrap(), old_r);
    assert_eq!(spectrum.kweight(), Some(&3.));
    let mut background = AUTOBK::new();
    background.rbkg = Some(1.5);
    spectrum
        .set_background_method(Some(BackgroundMethod::AUTOBK(background)))
        .unwrap();
    assert_eq!(spectrum.norm().unwrap(), norm);
    assert!(spectrum.chi().is_none());
    assert!(spectrum.r().is_none());
    spectrum.fft().unwrap();
    assert_ne!(spectrum.chi().unwrap(), chi);
}

#[test]
fn unsupported_selected_algorithms_never_fall_back_to_defaults() {
    let mut spectrum = sample();
    spectrum.fft().unwrap();
    spectrum
        .set_normalization_method(Some(NormalizationMethod::new_mback()))
        .unwrap();
    assert!(spectrum.fft().unwrap_err().to_string().contains("MBack"));
    assert!(matches!(
        spectrum.normalization,
        Some(NormalizationMethod::MBack(_))
    ));
    spectrum.set_normalization_method(None).unwrap();
    spectrum
        .set_background_method(Some(BackgroundMethod::new_ilpbkg()))
        .unwrap();
    assert!(spectrum.fft().unwrap_err().to_string().contains("ILPBkg"));
    assert!(matches!(
        spectrum.background,
        Some(BackgroundMethod::ILPBkg(_))
    ));
}

#[test]
fn replacing_data_invalidates_previous_stages() {
    let mut spectrum = sample();
    spectrum.fft().unwrap();
    let energy = spectrum.energy.clone().unwrap();
    let mu = spectrum.mu.clone().unwrap();
    spectrum.set_spectrum(energy, mu);
    assert!(spectrum.e0().is_none());
    assert!(spectrum.chi().is_none());
    assert!(spectrum.r().is_none());
    spectrum.fft().unwrap();
    assert!(spectrum.r().is_some());
}

#[test]
fn explicit_edge_step_survives_invalidation_and_serialization() {
    let mut spectrum = sample();
    let mut norm = PrePostEdge::new();
    norm.edge_step = Some(2.5);
    spectrum
        .set_normalization_method(Some(NormalizationMethod::PrePostEdge(norm)))
        .unwrap();
    spectrum.fft().unwrap();
    let json = serde_json::to_string(&spectrum).unwrap();
    let mut restored: Spectrum = serde_json::from_str(&json).unwrap();
    for spectrum in [&mut spectrum, &mut restored] {
        spectrum
            .set_e0(spectrum.e0().unwrap() + 0.25)
            .fft()
            .unwrap();
        assert_eq!(
            spectrum.normalization.as_ref().unwrap().get_edge_step(),
            Some(2.5)
        );
    }
}

#[test]
fn normalization_edge_override_propagates_to_background() {
    let mut spectrum = sample();
    spectrum.fft().unwrap();
    let edge = spectrum.e0().unwrap() + 0.5;
    let mut norm = PrePostEdge::new();
    norm.e0 = Some(edge);
    spectrum
        .set_normalization_method(Some(NormalizationMethod::PrePostEdge(norm)))
        .unwrap()
        .fft()
        .unwrap();
    assert_eq!(spectrum.e0(), Some(edge));
    if let Some(BackgroundMethod::AUTOBK(a)) = &spectrum.background {
        assert_eq!(a.ek0, Some(edge));
    }
}

#[test]
fn failed_transform_preserves_parameters_and_clears_results() {
    let mut spectrum = sample();
    spectrum.fft().unwrap();
    let mut params = XrayFFTF::new();
    params.nfft = Some(0);
    params.kweight = Some(3.);
    spectrum.set_fft(params);
    assert!(spectrum.fft().is_err());
    assert_eq!(spectrum.xftf.as_ref().unwrap().nfft, Some(0));
    assert_eq!(spectrum.xftf.as_ref().unwrap().kweight, Some(3.));
    assert!(spectrum.r().is_none());
    assert!(spectrum.chi().is_some());
}
