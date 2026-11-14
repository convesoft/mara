//! The `mara` command line.

mod cli;
mod debug_stats;

use std::ffi::OsString;
use std::process::ExitCode;

use mara_engine::work::WorkCounts;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let debug_stats = arguments.iter().any(|argument| argument == "--debug-stats");
    let mut work = WorkCounts::default();
    let status = cli::run(&arguments, &mut work);
    // The stats line comes last on stderr, also after rejected arguments.
    if debug_stats {
        eprintln!("{}", debug_stats::line(&work));
    }
    ExitCode::from(status)
}
