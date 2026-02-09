use std::alloc::{GlobalAlloc, Layout, System};
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use xraytsubaki::xafs::io::load_spectrum_QAS_trans;
use xraytsubaki::xafs::xasgroup::XASGroup;

struct CountingAlloc;

static ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static DEALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static REALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOC_BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        DEALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        REALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        if new_size > layout.size() {
            ALLOC_BYTES.fetch_add((new_size - layout.size()) as u64, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[derive(Debug, Clone, Copy)]
struct AllocStats {
    alloc_calls: u64,
    dealloc_calls: u64,
    realloc_calls: u64,
    alloc_bytes: u64,
    dealloc_bytes: u64,
}

fn reset_alloc_stats() {
    ALLOC_CALLS.store(0, Ordering::Relaxed);
    DEALLOC_CALLS.store(0, Ordering::Relaxed);
    REALLOC_CALLS.store(0, Ordering::Relaxed);
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    DEALLOC_BYTES.store(0, Ordering::Relaxed);
}

fn read_alloc_stats() -> AllocStats {
    AllocStats {
        alloc_calls: ALLOC_CALLS.load(Ordering::Relaxed),
        dealloc_calls: DEALLOC_CALLS.load(Ordering::Relaxed),
        realloc_calls: REALLOC_CALLS.load(Ordering::Relaxed),
        alloc_bytes: ALLOC_BYTES.load(Ordering::Relaxed),
        dealloc_bytes: DEALLOC_BYTES.load(Ordering::Relaxed),
    }
}

fn run_single(path: &str) -> Result<(f64, AllocStats), Box<dyn Error>> {
    reset_alloc_stats();
    let spectrum = load_spectrum_QAS_trans(path)?;
    let mut group = XASGroup::new();
    for _ in 0..100 {
        group.add_spectrum(spectrum.clone());
    }

    let start = Instant::now();
    group.normalize_seq()?.calc_background_seq()?.fft_seq()?;
    let elapsed_s = start.elapsed().as_secs_f64();
    let stats = read_alloc_stats();
    Ok((elapsed_s, stats))
}

fn run_parallel(path: &str) -> Result<(f64, AllocStats), Box<dyn Error>> {
    reset_alloc_stats();
    let spectrum = load_spectrum_QAS_trans(path)?;
    let mut group = XASGroup::new();
    for _ in 0..10_000 {
        group.add_spectrum(spectrum.clone());
    }

    let start = Instant::now();
    group.normalize_par()?.calc_background_par()?.fft_par()?;
    let elapsed_s = start.elapsed().as_secs_f64();
    let stats = read_alloc_stats();
    Ok((elapsed_s, stats))
}

fn print_row(name: &str, elapsed_s: f64, stats: AllocStats) {
    println!(
        "{name},elapsed_s={elapsed_s:.6},alloc_calls={},dealloc_calls={},realloc_calls={},alloc_bytes={},dealloc_bytes={}",
        stats.alloc_calls,
        stats.dealloc_calls,
        stats.realloc_calls,
        stats.alloc_bytes,
        stats.dealloc_bytes
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = format!("{}/tests/testfiles/Ru_QAS.dat", env!("CARGO_MANIFEST_DIR"));

    let (single_elapsed, single_stats) = run_single(&path)?;
    print_row(
        "xas_group_benchmark_single_alloc",
        single_elapsed,
        single_stats,
    );

    let (parallel_elapsed, parallel_stats) = run_parallel(&path)?;
    print_row(
        "xas_group_benchmark_parallel_alloc",
        parallel_elapsed,
        parallel_stats,
    );

    Ok(())
}
