//! Diagnostics keep no string alive once they are dropped (DES-DIAGNOSTICS in
//! `docs/validation.mara.md`): the memory a process holds does not grow with the number of
//! distinct invariant names it has reported. A binary of its own without the test harness, so
//! that nothing else allocates while it counts.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use mara_engine::config::parse_config;
use mara_engine::diagnostic::Diagnostic;
use mara_engine::evaluation::invariant_findings;
use mara_engine::facts::Value;
use mara_engine::snapshot::{Snapshot, SnapshotInputs};
use mara_engine::work::WorkCounts;

/// Counts the bytes allocated and not yet freed.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's obligations are those of `System.alloc`.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller's obligations are those of `System.dealloc`.
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const NAMES: usize = 1_000;
const NAME_BYTES: usize = 64;

/// The diagnostics of one row each of `NAMES` invariants with distinct names of `NAME_BYTES`
/// bytes, built by the evaluation's builder.
fn report(round: usize, snapshot: &Snapshot) -> Vec<Diagnostic> {
    let rows = [vec![Value::Symbol("no-such-mid".to_owned())]];
    (0..NAMES)
        .flat_map(|index| {
            let mut name = format!("r{round}-{index:04}-");
            name.extend(std::iter::repeat_n('x', NAME_BYTES - name.len()));
            let text = format!(
                "format_version = 1\nschema = \"s.toml\"\n[documents]\nformat_version = 1\n\
                 include = [\"*.mara.md\"]\n[rules]\nprograms = [\"r/*.dl\"]\n\
                 [invariants.{name}]\nprogram = \"r/p.dl\"\nrelation = \"r\"\n\
                 severity = \"error\"\nmessage = \"m\"\n"
            );
            let (config, diagnostics) = parse_config(text.as_bytes());
            assert_eq!(diagnostics, []);
            let entry = &config.rules.expect("rules").entries[0];
            let findings = invariant_findings(entry, &rows, snapshot);
            assert_eq!(findings[0].diagnostic.code, name);
            findings.into_iter().map(|finding| finding.diagnostic)
        })
        .collect()
}

fn main() {
    // nextest lists a binary's tests with libtest's `--list --format terse`, and its ignored tests
    // with `--ignored` added (https://nexte.st/docs/design/custom-test-harnesses/). Any other
    // arguments run the check, as nextest's `diagnostic_memory --exact --nocapture` and cargo's
    // plain run do.
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--list") {
        if !arguments.iter().any(|argument| argument == "--ignored") {
            println!("diagnostic_memory: test");
        }
        return;
    }
    let snapshot =
        SnapshotInputs::new(Vec::new(), None, None, None).build(&mut WorkCounts::default());
    // The first round sets up whatever the process keeps once, whatever the names.
    drop(report(0, &snapshot));
    let before = LIVE.load(Ordering::Relaxed);
    let diagnostics = report(1, &snapshot);
    let held = LIVE.load(Ordering::Relaxed) - before;
    assert!(
        held >= NAMES * NAME_BYTES,
        "the diagnostics of {NAMES} names hold only {held} bytes: the allocator does not count them"
    );
    drop(diagnostics);
    let after = LIVE.load(Ordering::Relaxed);
    // A table that kept each name would keep at least NAMES * NAME_BYTES bytes.
    assert!(
        after <= before + NAME_BYTES,
        "{} bytes stay allocated after the diagnostics of {NAMES} new names were dropped",
        after.saturating_sub(before)
    );
    println!("diagnostic memory: {held} bytes held by {NAMES} names, all freed");
}
