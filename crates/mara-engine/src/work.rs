//! Work counters (DES-PERFORMANCE-HARNESS in `docs/performance.mara.md`).

/// The work one request does.
///
/// The host creates one per request, passes it by `&mut` to every engine call that does counted
/// work, adds its own I/O and reports the total. The code that does a piece of work counts it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkCounts {
    /// Reads of input files' contents by the host. Stats and failed reads are none.
    pub file_reads: u64,
    /// Files hashed.
    pub hashes: u64,
    /// Documents parsed.
    pub parses: u64,
    /// Snapshots built.
    pub snapshot_builds: u64,
    /// Code files scanned for markers.
    pub code_scans: u64,
    /// Processes spawned by the host.
    pub subprocess_spawns: u64,
    /// Items and edges visited by built-in checks, one step each.
    pub check_steps: u64,
    /// Program evaluations.
    pub program_runs: u64,
}

impl WorkCounts {
    /// Every counter with its name, in a fixed order.
    pub fn fields(&self) -> [(&'static str, u64); 8] {
        [
            ("file_reads", self.file_reads),
            ("hashes", self.hashes),
            ("parses", self.parses),
            ("snapshot_builds", self.snapshot_builds),
            ("code_scans", self.code_scans),
            ("subprocess_spawns", self.subprocess_spawns),
            ("check_steps", self.check_steps),
            ("program_runs", self.program_runs),
        ]
    }
}
