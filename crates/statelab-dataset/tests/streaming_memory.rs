//! Proves the Dataset Explorer's streaming memory bound (§6.2 FROZEN).
//!
//! The README claims O(1) backend memory: generate one state, run it, summarize
//! it, emit the row, drop the trajectory. Re-reading the code and agreeing is not
//! evidence — a `Vec` accumulating somewhere would look identical at a glance.
//!
//! So this measures it. A tracking global allocator records peak live bytes, and
//! the assertion is the one that actually matters: **peak memory must not grow
//! with the dataset size.** A buffering implementation fails this immediately;
//! a streaming one passes with peak roughly flat.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use statelab_dataset::{for_each_summary, DatasetSpec};

/// Live bytes currently allocated, and the high-water mark.
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Tracking;

unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: Tracking = Tracking;

/// Streams `1..=count`, returning peak live bytes observed during the run.
fn peak_bytes_for_range(count: u64) -> usize {
    // Settle, then reset the high-water mark to "now" so the measurement covers
    // only the streaming run.
    let baseline = LIVE.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);

    let mut rows = 0u64;
    let processed = for_each_summary(
        DatasetSpec::Range {
            start: 1,
            end: count,
        },
        Some(1_000_000),
        |_row| {
            // Consume and immediately discard, exactly as an HTTP writer or an
            // IPC channel does. Deliberately retains nothing.
            rows += 1;
            true
        },
    );
    assert_eq!(processed, count);
    assert_eq!(rows, count);

    PEAK.load(Ordering::Relaxed).saturating_sub(baseline)
}

#[test]
fn peak_memory_does_not_grow_with_dataset_size() {
    // Warm up so one-off lazily-initialised allocations are not attributed to the
    // first measured run.
    let _ = peak_bytes_for_range(200);

    let small = peak_bytes_for_range(1_000);
    let large = peak_bytes_for_range(20_000);

    // 20x the items. If anything accumulated per-item, peak would grow roughly
    // 20x too. Streaming keeps it flat, so allow generous slack for allocator
    // noise and still catch any real accumulation.
    assert!(
        large <= small.max(4_096) * 3,
        "peak memory grew with dataset size: {small} bytes for 1,000 items vs \
         {large} bytes for 20,000 items — this suggests the dataset is being \
         buffered rather than streamed"
    );
}

#[test]
fn peak_memory_stays_small_in_absolute_terms() {
    let _ = peak_bytes_for_range(200);
    let peak = peak_bytes_for_range(20_000);

    // 20,000 Collatz trajectories, fully materialised, would be tens of MB. A few
    // hundred KB proves only a handful are alive at once.
    assert!(
        peak < 4 * 1024 * 1024,
        "peak of {peak} bytes for 20,000 items is too large to be one-at-a-time \
         streaming"
    );
}

#[test]
fn an_early_stop_does_not_process_the_whole_range() {
    // The sink's `false` return must actually halt generation — otherwise a
    // disconnected client would still cost the full sweep.
    let mut seen = 0u64;
    let processed = for_each_summary(
        DatasetSpec::Range {
            start: 1,
            end: 1_000_000,
        },
        Some(1_000_000),
        |_row| {
            seen += 1;
            seen < 10
        },
    );
    assert_eq!(seen, 10);
    assert_eq!(processed, 10);
}
