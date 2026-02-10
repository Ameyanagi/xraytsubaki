mod perf;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use nalgebra::DVector;
use perf::FlamegraphProfiler;
use xraytsubaki::xafs::fitting::{
    feffit_batch_with_options, feffpath, ff2chi, FeffBatchOptions, FeffBatchParallelMode,
    FeffFitDataset, FeffFlavor, FitVariable, FitVariables, PathParamSpec,
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

fn criterion_benchmark(c: &mut Criterion) {
    let template = fixture_dataset();
    let vars = initial_variables();
    let batch = vec![template; 10_000];

    let mut group = c.benchmark_group("feff_fitting_batch");
    group.throughput(Throughput::Elements(batch.len() as u64));

    let serial = FeffBatchOptions {
        parallel_mode: FeffBatchParallelMode::Serial,
        chunk_size: 256,
        max_threads: None,
    };
    group.bench_function("feff_batch_independent_serial_10k", |b| {
        b.iter(|| {
            let out = feffit_batch_with_options(&batch, &vars, &serial).unwrap();
            black_box(out.len())
        })
    });

    let rayon = FeffBatchOptions {
        parallel_mode: FeffBatchParallelMode::Rayon,
        chunk_size: 256,
        max_threads: None,
    };
    group.bench_function("feff_batch_independent_rayon_10k", |b| {
        b.iter(|| {
            let out = feffit_batch_with_options(&batch, &vars, &rayon).unwrap();
            black_box(out.len())
        })
    });

    group.finish();
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
