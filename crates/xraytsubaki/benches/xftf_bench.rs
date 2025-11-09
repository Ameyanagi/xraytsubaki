use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;
use xraytsubaki::xafs::io;

pub const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

/// Benchmark XFTF (X-ray Fourier Transform Forward) for 100,000 spectra in parallel
///
/// This benchmark measures the performance of the FFT operation which is a critical
/// component of XAFS data analysis. The XFTF transforms chi(k) to chi(R).
fn bench_xftf_100k_parallel(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let xafs_test_spectrum = io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = c.benchmark_group("xftf_massive_parallel");
    group.measurement_time(Duration::from_secs(60));
    group.sample_size(10);

    group.bench_function("100k_spectra_xftf_parallel", |b| {
        b.iter(|| {
            use rayon::prelude::*;

            // Process 100,000 spectra in parallel
            let results: Vec<_> = (0..100_000)
                .into_par_iter()
                .map(|_| {
                    let mut spectrum = xafs_test_spectrum.clone();
                    black_box(
                        spectrum
                            .normalize()
                            .unwrap()
                            .calc_background()
                            .unwrap()
                            .fft()
                            .unwrap()
                    );
                })
                .collect();

            black_box(results);
        });
    });

    group.finish();
}

/// Benchmark single spectrum XFTF for baseline measurement
fn bench_xftf_single(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let xafs_test_spectrum = io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = c.benchmark_group("xftf_single");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    group.bench_function("single_spectrum_xftf", |b| {
        b.iter(|| {
            let mut spectrum = xafs_test_spectrum.clone();
            black_box(
                spectrum
                    .normalize()
                    .unwrap()
                    .calc_background()
                    .unwrap()
                    .fft()
                    .unwrap()
            );
        });
    });

    group.finish();
}

/// Benchmark batch processing with different batch sizes
fn bench_xftf_batch_scaling(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let xafs_test_spectrum = io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = c.benchmark_group("xftf_batch_scaling");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

    for batch_size in [100, 1_000, 10_000].iter() {
        group.bench_function(format!("{}_spectra", batch_size), |b| {
            b.iter(|| {
                use rayon::prelude::*;

                let results: Vec<_> = (0..*batch_size)
                    .into_par_iter()
                    .map(|_| {
                        let mut spectrum = xafs_test_spectrum.clone();
                        black_box(
                            spectrum
                                .normalize()
                                .unwrap()
                                .calc_background()
                                .unwrap()
                                .fft()
                                .unwrap()
                        );
                    })
                    .collect();

                black_box(results);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_xftf_single,
    bench_xftf_batch_scaling,
    bench_xftf_100k_parallel
);

criterion_main!(benches);
