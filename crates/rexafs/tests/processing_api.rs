use rexafs::{io, process, process_with_options, ProcessOptions, Spectrum};

fn sample() -> Spectrum {
    io::read_qas_transmission(format!(
        "{}/tests/testfiles/Ru_QAS.dat",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn facade_matches_the_existing_staged_pipeline() {
    let mut spectrum = sample();
    let output = process(
        spectrum.energy.as_ref().unwrap().as_slice(),
        spectrum.mu.as_ref().unwrap().as_slice(),
    )
    .unwrap();
    spectrum
        .find_e0()
        .unwrap()
        .normalize()
        .unwrap()
        .calc_background()
        .unwrap()
        .fft()
        .unwrap();
    assert_eq!(output.e0, spectrum.get_e0().unwrap());
    assert_eq!(output.k, spectrum.k().unwrap());
    assert_eq!(output.chi, spectrum.chi().unwrap());
    assert_eq!(output.r, spectrum.get_r().unwrap().as_slice());
    assert_eq!(output.chir_mag, spectrum.get_chir_mag().unwrap().as_slice());
    assert_eq!(output.chir_re, spectrum.get_chir_real().unwrap().as_slice());
    assert_eq!(output.chir_im, spectrum.get_chir_imag().unwrap().as_slice());
    assert_eq!(output.k.len(), output.chi.len());
    assert_eq!(output.r.len(), output.chir_mag.len());
    for ((mag, re), im) in output
        .chir_mag
        .iter()
        .zip(&output.chir_re)
        .zip(&output.chir_im)
    {
        assert!((mag - re.hypot(*im)).abs() < 1e-10);
    }
}

#[test]
fn invalid_arrays_return_errors_before_processing() {
    for (energy, mu) in [
        (vec![], vec![]),
        (vec![2., 1., 3.], vec![1.]), // previously could panic in sorting
        (vec![1., 2., 2.], vec![1., 2., 3.]),
        (vec![1., f64::NAN, 3.], vec![1., 2., 3.]),
        (vec![1., 2., 3.], vec![1., f64::INFINITY, 3.]),
    ] {
        assert!(process(&energy, &mu).is_err());
    }
}

#[test]
fn edge_override_is_validated_and_used() {
    let spectrum = sample();
    let energy = spectrum.energy.as_ref().unwrap().as_slice();
    let mu = spectrum.mu.as_ref().unwrap().as_slice();
    let automatic = process(energy, mu).unwrap();
    let explicit = process_with_options(
        energy,
        mu,
        ProcessOptions {
            e0: Some(automatic.e0 + 0.25),
        },
    )
    .unwrap();
    assert_eq!(explicit.e0, automatic.e0 + 0.25);
    for e0 in [
        f64::NAN,
        f64::INFINITY,
        energy[0] - 1.,
        energy[energy.len() - 1] + 1.,
    ] {
        assert!(process_with_options(energy, mu, ProcessOptions { e0: Some(e0) }).is_err());
    }
}
