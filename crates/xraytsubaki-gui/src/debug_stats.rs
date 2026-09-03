//! Opt-in interaction diagnostics (`XTS_DEBUG_STATS=1`).
//!
//! Cheap atomic counters fed from the plot cards' pointer handlers and the
//! handle layer's paint closure; `StudioApp::start_debug_stats` prints a
//! one-line summary per second to stderr together with ruviz-gpui's frame
//! and presentation statistics for the main stage plot. Off by default:
//! every entry point is a single relaxed load when the variable is unset.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static ENABLED: OnceLock<bool> = OnceLock::new();
static BASE: OnceLock<Instant> = OnceLock::new();
static POINTER_EVENTS: AtomicU64 = AtomicU64::new(0);
/// Nanoseconds (since `BASE`) of the last pointer event not yet followed by
/// a paint; 0 when none is pending.
static PENDING_POINTER_NS: AtomicU64 = AtomicU64::new(0);
static PAINTS: AtomicU64 = AtomicU64::new(0);
static LATENCY_SUM_NS: AtomicU64 = AtomicU64::new(0);
static LATENCY_MAX_NS: AtomicU64 = AtomicU64::new(0);
static LATENCY_N: AtomicU64 = AtomicU64::new(0);

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var_os("XTS_DEBUG_STATS").is_some())
}

fn now_ns() -> u64 {
    BASE.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

/// A pointer event reached a plot card.
pub fn pointer_event() {
    if !enabled() {
        return;
    }
    POINTER_EVENTS.fetch_add(1, Ordering::Relaxed);
    PENDING_POINTER_NS.store(now_ns().max(1), Ordering::Relaxed);
}

/// The plot card's handle layer painted a frame.
pub fn painted() {
    if !enabled() {
        return;
    }
    PAINTS.fetch_add(1, Ordering::Relaxed);
    let pending = PENDING_POINTER_NS.swap(0, Ordering::Relaxed);
    if pending != 0 {
        let lat = now_ns().saturating_sub(pending);
        LATENCY_SUM_NS.fetch_add(lat, Ordering::Relaxed);
        LATENCY_MAX_NS.fetch_max(lat, Ordering::Relaxed);
        LATENCY_N.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct Snapshot {
    pub pointer_events: u64,
    pub paints: u64,
    pub latency_avg_ms: f64,
    pub latency_max_ms: f64,
    pub latency_n: u64,
}

/// Read and reset the interval counters.
pub fn take() -> Snapshot {
    let n = LATENCY_N.swap(0, Ordering::Relaxed);
    let sum = LATENCY_SUM_NS.swap(0, Ordering::Relaxed);
    let max = LATENCY_MAX_NS.swap(0, Ordering::Relaxed);
    Snapshot {
        pointer_events: POINTER_EVENTS.swap(0, Ordering::Relaxed),
        paints: PAINTS.swap(0, Ordering::Relaxed),
        latency_avg_ms: if n > 0 {
            sum as f64 / n as f64 / 1e6
        } else {
            0.0
        },
        latency_max_ms: max as f64 / 1e6,
        latency_n: n,
    }
}
