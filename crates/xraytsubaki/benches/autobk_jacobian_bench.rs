use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use xraytsubaki::xafs::io;

pub const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

/// Benchmark the full AUTOBK background removal with Jacobian optimization
///
/// This benchmark measures the end-to-end performance of the AUTOBK algorithm
/// with the precomputed basis Jacobian and allocation elimination optimizations.
///
/// Expected improvements from Phase 1 optimizations:
/// - 25-35% total speedup
/// - 50% reduction in memory allocations
/// - ~12% reduction in Jacobian computation time per iteration
fn bench_autobk_full_optimization(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let xafs_test_spectrum = io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = c.benchmark_group("autobk_optimization");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    group.bench_function("full_background_removal", |b| {
        b.iter(|| {
            let mut spectrum = xafs_test_spectrum.clone();
            black_box(
                spectrum
                    .normalize()
                    .unwrap()
                    .calc_background()
                    .unwrap()
            );
        });
    });

    group.finish();
}

/// Benchmark specifically the Jacobian computation performance
///
/// This would require exposing internal methods for detailed profiling.
/// For now, we measure the full background calculation which includes
/// multiple Jacobian evaluations during LM optimization.
fn bench_autobk_jacobian_intensive(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let xafs_test_spectrum = io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = c.benchmark_group("jacobian_performance");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(15);

    // Test with different spectrum configurations to validate consistent speedup
    for spectrum_type in ["standard", "normalized"].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(spectrum_type),
            spectrum_type,
            |b, &case| {
                b.iter(|| {
                    let mut spectrum = xafs_test_spectrum.clone();
                    match case {
                        "standard" => {
                            black_box(
                                spectrum
                                    .normalize()
                                    .unwrap()
                                    .calc_background()
                                    .unwrap()
                            );
                        }
                        "normalized" => {
                            spectrum.normalize().unwrap();
                            black_box(spectrum.calc_background().unwrap());
                        }
                        _ => unreachable!(),
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark batch processing to measure allocation efficiency
///
/// Processing multiple spectra in sequence helps identify allocation patterns
/// and validates that the optimization scales linearly.
fn bench_autobk_batch_processing(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let xafs_test_spectrum = io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = c.benchmark_group("batch_processing");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

    for batch_size in [1, 10, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    for _ in 0..size {
                        let mut spectrum = xafs_test_spectrum.clone();
                        black_box(
                            spectrum
                                .normalize()
                                .unwrap()
                                .calc_background()
                                .unwrap()
                        );
                    }
                });
            },
        );
    }

    group.finish();
}

/// Memory allocation benchmark
///
/// Measures the impact of pre-allocated buffers and elimination of extend() calls.
/// Expected: 50% reduction in allocations (300 per iteration → ~150).
fn bench_autobk_memory_efficiency(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let xafs_test_spectrum = io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = c.benchmark_group("memory_efficiency");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("single_spectrum_allocations", |b| {
        b.iter(|| {
            let mut spectrum = xafs_test_spectrum.clone();
            black_box(
                spectrum
                    .normalize()
                    .unwrap()
                    .calc_background()
                    .unwrap()
            );
        });
    });

    group.finish();
}

/// Large-scale parallel benchmark for 100,000 spectra
///
/// This benchmark tests the scalability of the AUTOBK optimization when
/// processing a massive number of spectra in parallel using Rayon.
///
/// Expected behavior:
/// - Near-linear scaling with CPU core count
/// - Consistent per-spectrum performance
/// - Efficient memory utilization across parallel workers
fn bench_massive_parallel_processing(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let xafs_test_spectrum = io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = c.benchmark_group("massive_parallel");
    group.measurement_time(Duration::from_secs(60));  // Extended time for large-scale test
    group.sample_size(10);  // Fewer samples due to computational cost

    group.bench_function("100k_spectra_parallel", |b| {
        b.iter(|| {
            use rayon::prelude::*;

            // Process 100,000 spectra in parallel using Rayon
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
                    );
                })
                .collect();

            black_box(results);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_autobk_full_optimization,
    bench_autobk_jacobian_intensive,
    bench_autobk_batch_processing,
    bench_autobk_memory_efficiency,
    bench_massive_parallel_processing
);

criterion_main!(benches);
