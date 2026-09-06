use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

use rexafs::xafs::{self, xasgroup::XASGroup};

pub const TOP_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn build_group(sample: &xafs::xasspectrum::XASSpectrum, n_spectra: usize) -> XASGroup {
    let mut group = XASGroup::new();
    for _ in 0..n_spectra {
        group.add_spectrum(sample.clone());
    }
    group
}

fn criterion_benchmark(c: &mut Criterion) {
    let path = format!("{TOP_DIR}/tests/testfiles/Ru_QAS.dat");
    let sample = xafs::io::load_spectrum_QAS_trans(&path).unwrap();

    // Keep legacy benchmark ID for regression scripts and baselines.
    let mut legacy_group_10k = build_group(&sample, 10_000);
    c.bench_function("xas_group_benchmark_parallel", |b| {
        b.iter(|| {
            let _ = legacy_group_10k
                .normalize_par()
                .unwrap()
                .calc_background_par()
                .unwrap()
                .fft_par()
                .unwrap();
            black_box(())
        })
    });

    let mut bench_group = c.benchmark_group("xas_group_par_matched");
    for n_spectra in [100usize, 10_000usize] {
        let mut group = build_group(&sample, n_spectra);
        bench_group.throughput(Throughput::Elements(n_spectra as u64));
        bench_group.bench_with_input(
            BenchmarkId::from_parameter(n_spectra),
            &n_spectra,
            |b, _| {
                b.iter(|| {
                    let _ = group
                        .normalize_par()
                        .unwrap()
                        .calc_background_par()
                        .unwrap()
                        .fft_par()
                        .unwrap();
                    black_box(())
                })
            },
        );
    }

    bench_group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = criterion_benchmark
}

criterion_main!(benches);
