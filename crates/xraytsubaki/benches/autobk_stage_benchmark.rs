mod perf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use perf::FlamegraphProfiler;
use xraytsubaki::xafs::background::{AUTOBKSolver, AUTOBK};
use xraytsubaki::xafs::normalization::{NormalizationMethod, PrePostEdge};

pub const TOP_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn build_fixture() -> (
    nalgebra::DVector<f64>,
    nalgebra::DVector<f64>,
    Option<NormalizationMethod>,
) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let mut spectrum = xraytsubaki::xafs::io::load_spectrum_QAS_trans(&path).unwrap();
    spectrum
        .set_normalization_method(Some(NormalizationMethod::PrePostEdge(PrePostEdge::new())))
        .unwrap()
        .normalize()
        .unwrap();

    (
        spectrum.energy.clone().unwrap(),
        spectrum.mu.clone().unwrap(),
        spectrum.normalization.clone(),
    )
}

fn criterion_benchmark(c: &mut Criterion) {
    let (energy, mu, normalization) = build_fixture();

    c.bench_function("autobk_stage_legacy_lm", |b| {
        b.iter(|| {
            let mut autobk = AUTOBK::new();
            autobk.solver = Some(AUTOBKSolver::LegacyLm);

            let mut norm = normalization.clone();
            autobk.calc_background(&energy, &mu, &mut norm).unwrap();

            black_box(autobk.get_chi().unwrap().len())
        })
    });

    c.bench_function("autobk_stage_linear_direct", |b| {
        b.iter(|| {
            let mut autobk = AUTOBK::new();
            autobk.solver = Some(AUTOBKSolver::LinearDirect);

            let mut norm = normalization.clone();
            autobk.calc_background(&energy, &mu, &mut norm).unwrap();

            black_box(autobk.get_chi().unwrap().len())
        })
    });
}

fn custom() -> Criterion {
    let base = Criterion::default().sample_size(20);
    let enable_profiler = std::env::args()
        .any(|arg| arg == "--profile-time" || arg.starts_with("--profile-time="));
    if enable_profiler {
        base.with_profiler(FlamegraphProfiler::new(1000))
    } else {
        base
    }
}

criterion_group! {
    name = benches;
    config = custom();
    targets = criterion_benchmark
}

criterion_main!(benches);
