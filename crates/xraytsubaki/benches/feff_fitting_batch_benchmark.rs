mod perf;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use nalgebra::DVector;
use perf::FlamegraphProfiler;
use std::num::NonZeroUsize;
use xraytsubaki::xafs::fitting::{
    feffit_independent, feffit_joint_with_options, feffpath, ff2chi, FeffBatchOptions,
    FeffFitDataset, FeffFitJacobianMode, FeffFitOptions, FeffFlavor, FitVariable, FitVariables,
    PathParamSpec,
};

pub const TOP_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn fixture_dataset() -> FeffFitDataset {
    let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
    let mut path = feffpath(&pathfile, FeffFlavor::Feff85L).unwrap();
    path.s02 = PathParamSpec::Expression("amp".to_string());
    path.e0 = PathParamSpec::Expression("de0".to_string());
    path.sigma2 = PathParamSpec::Expression("sig2".to_string());
    path.deltar = PathParamSpec::Expression("dr".to_string());

    let k = DVector::from_iterator(140, (0..140).map(|i| 0.05 * (i as f64 + 1.0)));
    let mut truth = FitVariables::new();
    truth.insert("amp", FitVariable::new(0.9, false));
    truth.insert("de0", FitVariable::new(1.1, false));
    truth.insert("sig2", FitVariable::new(0.0031, false));
    truth.insert("dr", FitVariable::new(0.01, false));
    let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();

    FeffFitDataset::new()
        .data(&k, &synthetic.chi)
        .epsilon_k(1.0)
        .add_path(path)
}

fn initial_variables() -> FitVariables {
    let mut initial = FitVariables::new();
    initial.insert("amp", FitVariable::new(0.95, true));
    initial.insert("de0", FitVariable::new(0.0, true));
    initial.insert(
        "sig2",
        FitVariable::new(0.002, true).with_bounds(Some(0.0), Some(0.02)),
    );
    initial.insert("dr", FitVariable::new(0.0, true));
    initial
}

fn sparse_joint_fixture(count: usize) -> (Vec<FeffFitDataset>, FitVariables) {
    let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
    let base_path = feffpath(&pathfile, FeffFlavor::Feff85L).unwrap();
    let k = DVector::from_iterator(120, (0..120).map(|i| 0.05 * (i as f64 + 1.0)));
    let mut datasets = Vec::with_capacity(count);
    let mut initial = FitVariables::new();

    for idx in 0..count {
        let name = format!("amp_{idx}");
        let mut path = base_path.clone();
        path.s02 = PathParamSpec::Expression(name.clone());

        let mut truth = FitVariables::new();
        truth.insert(
            name.clone(),
            FitVariable::new(0.75 + idx as f64 * 0.015, false),
        );
        let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();

        datasets.push(
            FeffFitDataset::new()
                .data(&k, &synthetic.chi)
                .epsilon_k(1.0)
                .add_path(path),
        );
        initial.insert(name, FitVariable::new(0.95, true));
    }

    (datasets, initial)
}

fn criterion_benchmark(c: &mut Criterion) {
    let template = fixture_dataset();
    let vars = initial_variables();
    let batch = vec![template; 64];

    let mut group = c.benchmark_group("feff_fitting_batch");
    group.throughput(Throughput::Elements(batch.len() as u64));

    let trust_region_serial = FeffBatchOptions::sequential()
        .with_chunk_size(NonZeroUsize::new(32).expect("nonzero constant"))
        .with_solver_options(FeffFitOptions::trust_region());
    group.bench_function("feff_batch_trust_region_serial_64", |b| {
        b.iter(|| {
            let out = feffit_independent(&batch, &vars, &trust_region_serial);
            let ok_count = out.iter().filter(|item| item.is_ok()).count();
            black_box(ok_count)
        })
    });

    let lm_serial = FeffBatchOptions::sequential()
        .with_chunk_size(NonZeroUsize::new(32).expect("nonzero constant"))
        .with_solver_options(FeffFitOptions::levenberg_marquardt());
    group.bench_function("feff_batch_lm_serial_64", |b| {
        b.iter(|| {
            let out = feffit_independent(&batch, &vars, &lm_serial);
            let ok_count = out.iter().filter(|item| item.is_ok()).count();
            black_box(ok_count)
        })
    });

    let trust_region_rayon = FeffBatchOptions::parallel()
        .with_chunk_size(NonZeroUsize::new(32).expect("nonzero constant"))
        .with_solver_options(FeffFitOptions::trust_region());
    group.bench_function("feff_batch_trust_region_rayon_64", |b| {
        b.iter(|| {
            let out = feffit_independent(&batch, &vars, &trust_region_rayon);
            let ok_count = out.iter().filter(|item| item.is_ok()).count();
            black_box(ok_count)
        })
    });

    let lm_rayon = FeffBatchOptions::parallel()
        .with_chunk_size(NonZeroUsize::new(32).expect("nonzero constant"))
        .with_solver_options(FeffFitOptions::levenberg_marquardt());
    group.bench_function("feff_batch_lm_rayon_64", |b| {
        b.iter(|| {
            let out = feffit_independent(&batch, &vars, &lm_rayon);
            let ok_count = out.iter().filter(|item| item.is_ok()).count();
            black_box(ok_count)
        })
    });

    group.finish();

    let (joint_datasets, joint_vars) = sparse_joint_fixture(8);
    let mut joint_group = c.benchmark_group("feff_fitting_joint");
    joint_group.throughput(Throughput::Elements(joint_datasets.len() as u64));

    let trust_region_sparse =
        FeffFitOptions::trust_region().with_jacobian_mode(FeffFitJacobianMode::Sparse);
    joint_group.bench_function("feff_joint_trust_region_sparse_8", |b| {
        b.iter(|| {
            let result =
                feffit_joint_with_options(&joint_datasets, &joint_vars, &trust_region_sparse)
                    .unwrap();
            black_box(result.chi_square)
        })
    });

    let lm = FeffFitOptions::levenberg_marquardt();
    joint_group.bench_function("feff_joint_lm_dense_8", |b| {
        b.iter(|| {
            let result = feffit_joint_with_options(&joint_datasets, &joint_vars, &lm).unwrap();
            black_box(result.chi_square)
        })
    });

    joint_group.finish();
}

fn custom() -> Criterion {
    let base = Criterion::default().sample_size(10);
    let enable_profiler =
        std::env::args().any(|arg| arg == "--profile-time" || arg.starts_with("--profile-time="));
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
