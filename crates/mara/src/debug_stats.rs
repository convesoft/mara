//! The `--debug-stats` option (DES-PERFORMANCE-HARNESS in `docs/performance.mara.md`).

use std::fmt::Write as _;

use mara_engine::work::WorkCounts;

/// Version of the stats line.
const FORMAT_VERSION: u32 = 1;

/// The line written to stderr after the request: a JSON object holding `format_version` and
/// every counter by name.
pub fn line(work: &WorkCounts) -> String {
    let mut line = format!("{{\"format_version\":{FORMAT_VERSION}");
    for (name, value) in work.fields() {
        write!(line, ",\"{name}\":{value}").expect("writing to a String cannot fail");
    }
    line.push('}');
    line
}
