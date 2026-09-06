//! Athena project (.prj) import/export tests.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use rexafs::prelude::*;

const TOP_DIR: &str = env!("CARGO_MANIFEST_DIR");
const TMP_DIR: &str = env!("CARGO_TARGET_TMPDIR");

fn testfile(name: &str) -> PathBuf {
    PathBuf::from(TOP_DIR).join("tests/testfiles").join(name)
}

fn args_map(group: &AthenaGroup) -> BTreeMap<String, String> {
    group
        .args
        .iter()
        .map(|(k, v)| (k.clone(), v.to_string()))
        .collect()
}

fn decompress(bytes: &[u8]) -> String {
    let mut text = String::new();
    GzDecoder::new(bytes).read_to_string(&mut text).unwrap();
    text
}

#[test]
fn athena_read_ru_qas_project() {
    let project = AthenaProject::read(testfile("Ru_QAS_athena.prj")).unwrap();
    assert_eq!(project.version, "0.9.26");
    assert_eq!(project.groups.len(), 1);
    assert!(project.journal.is_empty());

    let group = &project.groups[0];
    assert_eq!(group.tag, "jimwk");
    assert_eq!(group.label, "Ru_QAS.dat");
    assert_eq!(group.x.len(), 645);
    assert_eq!(group.y.len(), 645);
    assert_eq!(group.i0.as_ref().map(Vec::len), Some(645));
    assert_eq!(group.signal.as_ref().map(Vec::len), Some(645));
    assert!(group.stddev.is_none());
    assert!((group.x[0] - 21912.253421).abs() < 1e-9);
    assert!(!group.is_chi());

    let p = &group.params;
    assert_eq!(p.e0, Some(22118.8));
    assert_eq!(p.rbkg, Some(1.0));
    assert_eq!(p.fft_kmin, Some(2.0));
    assert_eq!(p.fft_kmax, Some(15.0));
    assert!((p.nor2.unwrap() - 944.53).abs() < 0.01);
    assert_eq!(p.pre1, Some(-200.0));
    assert_eq!(p.pre2, Some(-65.0));
    assert_eq!(p.nor1, Some(25.0));
    assert_eq!(p.nnorm, Some(2));
    assert_eq!(p.spl1, Some(0.0));
    assert_eq!(p.spl2, Some(15.777));
    assert_eq!(p.clamp1, Some(0));
    assert_eq!(p.clamp2, Some(24));
    assert_eq!(p.nclamp, Some(5));
    assert_eq!(p.bkg_kw, Some(1));
    assert_eq!(p.bkg_kwindow.as_deref(), Some("hanning"));
    assert_eq!(p.fft_kwindow.as_deref(), Some("hanning"));
    assert_eq!(p.fft_kw, None);
    assert_eq!(p.recordtype.as_deref(), Some("mu(E)"));
    assert_eq!(p.importance, Some(1.0));
    assert_eq!(p.mark, Some(false));
    assert_eq!(p.fixstep, Some(false));
    assert!((p.step.unwrap() - 0.8614324).abs() < 1e-9);

    // Perl escaping in the `file` key is resolved.
    assert_eq!(
        group.arg_str("file"),
        Some(r"\\Mac\Home\rust\xraytsubaki\tests\testfiles\Ru_QAS.dat")
    );
    // Empty Perl array ref survives.
    assert_eq!(group.arg("titles"), Some(&AthenaValue::List(vec![])));

    let mut spectrum = group.to_spectrum().unwrap();
    assert_eq!(spectrum.name.as_deref(), Some("Ru_QAS.dat"));
    assert_eq!(spectrum.e0(), Some(22118.8));
    spectrum.normalize().unwrap();
    let norm = spectrum.normalization.as_ref().unwrap();
    assert_eq!(norm.get_e0(), Some(22118.8));
    assert!(norm.get_edge_step().unwrap() > 0.5);
    match norm {
        NormalizationMethod::PrePostEdge(ppe) => {
            assert_eq!(ppe.norm_polyorder, Some(1));
            assert_eq!(ppe.pre_edge_start, Some(-200.0));
            assert_eq!(ppe.norm_end, p.nor2);
        }
        _ => panic!("expected PrePostEdge"),
    }
    match spectrum.background.as_ref().unwrap() {
        BackgroundMethod::AUTOBK(autobk) => {
            assert_eq!(autobk.rbkg, Some(1.0));
            assert_eq!(autobk.kmax, Some(15.777));
            assert_eq!(autobk.clamp_hi, Some(24));
            assert_eq!(autobk.window, FTWindow::Hanning);
        }
        _ => panic!("expected AUTOBK"),
    }
    let xftf = spectrum.xftf.as_ref().unwrap();
    assert_eq!(xftf.kmin, Some(2.0));
    assert_eq!(xftf.kmax, Some(15.0));
    assert_eq!(xftf.window, Some(FTWindow::Hanning));
    assert_eq!(xftf.kweight, Some(2.0));
    let xftr = spectrum.xftr.as_ref().unwrap();
    assert_eq!(xftr.rmin, Some(1.0));
    assert_eq!(xftr.rmax, Some(3.0));
}

#[test]
fn athena_roundtrip_preserves_everything() {
    let original = AthenaProject::read(testfile("Ru_QAS_athena.prj")).unwrap();

    let mut buffer: Vec<u8> = Vec::new();
    original.write_to(&mut buffer).unwrap();

    let text = decompress(&buffer);
    assert!(text.starts_with("# Athena project file -- Demeter version 0.9.26"));
    assert_eq!(text.matches("[record]").count(), original.groups.len());
    assert!(text.trim_end().ends_with("# End:"));
    let body = text.split("# Local Variables:").next().unwrap();
    assert!(body.trim_end().ends_with("1;"));

    let reread = AthenaProject::read_from(&mut buffer.as_slice()).unwrap();
    assert_eq!(reread.version, original.version);
    assert_eq!(reread.journal, original.journal);
    assert_eq!(reread.groups.len(), original.groups.len());
    for (a, b) in original.groups.iter().zip(&reread.groups) {
        assert_eq!(a.tag, b.tag);
        assert_eq!(a.label, b.label);
        assert_eq!(args_map(a), args_map(b));
        assert_eq!(a.args, b.args, "arg order must be preserved");
        assert_eq!(a.x, b.x);
        assert_eq!(a.y, b.y);
        assert_eq!(a.i0, b.i0);
        assert_eq!(a.signal, b.signal);
        assert_eq!(a.stddev, b.stddev);
        assert_eq!(a.params, b.params);
    }

    // `$old_group`/`@args` lines are byte-identical to the original Athena
    // text (data arrays are compared numerically above: Perl prints tiny
    // values as `e-006`, we print `e-6`).
    let original_text = decompress(&std::fs::read(testfile("Ru_QAS_athena.prj")).unwrap());
    let group_body = |t: &str| {
        t.lines()
            .filter(|l| l.starts_with("$old_group") || l.starts_with("@args"))
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(group_body(&text), group_body(&original_text));
    assert!(text.contains("'-4.25504672699857e-6'"));
}

#[test]
fn athena_typed_params_win_on_write() {
    let mut project = AthenaProject::read(testfile("Ru_QAS_athena.prj")).unwrap();
    project.groups[0].params.rbkg = Some(1.2);
    project.groups[0].params.fft_kw = Some(3.0);
    project.groups[0].label = "renamed".to_string();
    project.journal = vec!["first line".into(), "it's got a quote".into()];

    let mut buffer: Vec<u8> = Vec::new();
    project.write_to(&mut buffer).unwrap();
    let reread = AthenaProject::read_from(&mut buffer.as_slice()).unwrap();
    let g = &reread.groups[0];
    assert_eq!(g.params.rbkg, Some(1.2));
    assert_eq!(g.params.fft_kw, Some(3.0));
    assert_eq!(g.label, "renamed");
    assert_eq!(g.arg_str("label"), Some("renamed"));
    // `bkg_rbkg` was bare in the original, so it stays bare.
    assert_eq!(g.arg("bkg_rbkg"), Some(&AthenaValue::bare("1.2")));
    // new keys are appended at the end
    assert_eq!(g.args.last().unwrap().0, "fft_kw");
    assert_eq!(reread.journal, project.journal);
}

#[test]
fn athena_export_from_scratch() {
    let mut spectrum = io::load_spectrum_QAS_trans(testfile("Ru_QAS.dat")).unwrap();
    spectrum.set_name("Ru_QAS");
    spectrum.normalize().unwrap();
    let e0 = spectrum
        .normalization
        .as_ref()
        .and_then(|n| n.get_e0())
        .unwrap();
    let edge_step = spectrum
        .normalization
        .as_ref()
        .and_then(|n| n.get_edge_step())
        .unwrap();

    let project = AthenaProject::from_spectra(std::slice::from_ref(&spectrum)).unwrap();
    assert_eq!(project.groups.len(), 1);
    let g = &project.groups[0];
    assert_eq!(g.tag.len(), 5);
    assert_eq!(g.label, "Ru_QAS");
    assert_eq!(g.params.e0, Some(e0));
    assert_eq!(g.params.step, Some(edge_step));
    assert_eq!(g.params.nnorm, Some(3));
    assert_eq!(g.arg_str("recordtype"), Some("mu(E)"));
    assert_eq!(g.arg_str("datatype"), Some("xmu"));
    assert_eq!(g.arg_str("is_xmu"), Some("1"));
    assert_eq!(g.arg_str("bkg_clamp2"), Some("24"));
    assert_eq!(g.arg_str("fft_kwindow"), Some("kaiser-bessel"));
    assert_eq!(g.arg_str("npts"), Some(g.x.len().to_string().as_str()));
    assert!(g.arg_str("bkg_nvict").is_none());

    let path = PathBuf::from(TMP_DIR).join("athena_export_from_scratch.prj");
    project.write(&path).unwrap();
    let reread = AthenaProject::read(&path).unwrap();
    assert_eq!(reread.groups.len(), 1);
    let rg = &reread.groups[0];
    assert_eq!(rg.label, "Ru_QAS");
    let energy = spectrum.raw_energy.as_ref().unwrap();
    let mu = spectrum.raw_mu.as_ref().unwrap();
    assert_eq!(rg.x, energy.iter().copied().collect::<Vec<_>>());
    assert_eq!(rg.y, mu.iter().copied().collect::<Vec<_>>());
    assert_eq!(rg.params.e0, Some(e0));
    assert_eq!(args_map(rg), args_map(g));

    let spectra = reread.to_spectra().unwrap();
    assert_eq!(spectra[0].e0(), Some(e0));
    assert_eq!(spectra[0].raw_energy, spectrum.raw_energy);
}

#[test]
fn athena_rejects_non_project() {
    let err = AthenaProject::read_from(&mut "hello world\n".as_bytes()).unwrap_err();
    assert!(matches!(
        err,
        rexafs::xafs::errors::IOError::NotAthenaProject { .. }
    ));
    let err = AthenaProject::read(testfile("does_not_exist.prj")).unwrap_err();
    assert!(matches!(
        err,
        rexafs::xafs::errors::IOError::FileNotFound { .. }
    ));
}

/// Parity with Larch's `read_athena`. Needs `uv` and the Python venv under
/// `tests/pythonscript`; run manually with
/// `cargo test -p rexafs athena_larch -- --ignored`.
#[test]
#[ignore]
fn athena_larch_reads_our_export() {
    let mut spectrum = io::load_spectrum_QAS_trans(testfile("Ru_QAS.dat")).unwrap();
    spectrum.set_name("Ru_QAS");
    spectrum.normalize().unwrap();
    let e0 = spectrum
        .normalization
        .as_ref()
        .and_then(|n| n.get_e0())
        .unwrap();
    let npts = spectrum.raw_energy.as_ref().unwrap().len();

    let project = AthenaProject::from_spectra(std::slice::from_ref(&spectrum)).unwrap();
    let path = PathBuf::from(TMP_DIR).join("athena_larch_parity.prj");
    project.write(&path).unwrap();

    let script = format!(
        r#"
from larch.io import read_athena
p = read_athena({path:?})
names = list(p.groups.keys()) if hasattr(p, 'groups') else [n for n in dir(p) if not n.startswith('_')]
g = p.groups['Ru_QAS'] if hasattr(p, 'groups') else getattr(p, 'Ru_QAS')
print('LABEL', g.label)
print('NPTS', len(g.energy))
print('E0', g.athena_params.bkg.e0)
print('PREEDGE_E0', g.e0)
print('STEP', g.edge_step)
"#,
        path = path.display()
    );
    let output = std::process::Command::new("uv")
        .args(["run", "--project"])
        .arg(PathBuf::from(TOP_DIR).join("tests/pythonscript"))
        .args(["python", "-c", &script])
        .output()
        .expect("failed to run uv");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "larch failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let field = |name: &str| -> String {
        stdout
            .lines()
            .find_map(|l| l.strip_prefix(name).map(|v| v.trim().to_string()))
            .unwrap_or_else(|| panic!("missing {name} in larch output:\n{stdout}"))
    };
    assert_eq!(field("LABEL"), "Ru_QAS");
    assert_eq!(field("NPTS").parse::<usize>().unwrap(), npts);
    assert!((field("E0").parse::<f64>().unwrap() - e0).abs() < 1e-6);
    let larch_e0: f64 = field("PREEDGE_E0").parse().unwrap();
    assert!(
        (larch_e0 - e0).abs() < 1.0,
        "larch pre_edge e0 {larch_e0} vs ours {e0}"
    );
    let step: f64 = field("STEP").parse().unwrap();
    assert!(step > 0.5, "larch edge step {step}");
}
