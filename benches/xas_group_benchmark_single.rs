use criterion::{black_box, criterion_group, criterion_main, Criterion};

use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

pub const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");
pub const PARAM_LOADTXT: ReaderParams = ReaderParams {
    comments: Some(b'#'),
    delimiter: Delimiter::WhiteSpace,
    skip_footer: None,
    skip_header: None,
    usecols: None,
    max_rows: None,
    row_format: true,
};
pub const TEST_TOL: f64 = 1e-16;

use xraytsubaki::xafs::xasgroup::XASGroup;

fn criterion_benchmark(c: &mut Criterion) {
    let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
    let mut xafs_test_spectrum = xraytsubaki::xafs::io::load_spectrum_QAS_trans(&path).unwrap();

    let mut group = XASGroup::new();

    for _ in 0..100 {
        group.add_spectrum(xafs_test_spectrum.clone());
    }

    c.bench_function("normalize", |b| {
        b.iter(|| {
            black_box(
                group
                    .normalize_seq()
                    .unwrap()
                    .calc_background_seq()
                    .unwrap()
                    .fft_seq(),
            );
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = criterion_benchmark
}

criterion_main!(benches);
