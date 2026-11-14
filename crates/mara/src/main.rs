//! The `mara` command line.

mod debug_stats;

use std::process::ExitCode;

use mara_engine::work::WorkCounts;

fn main() -> ExitCode {
    let mut debug_stats = false;
    let mut undeclared = None;
    for argument in std::env::args_os().skip(1) {
        if argument == "--debug-stats" {
            debug_stats = true;
        } else if undeclared.is_none() {
            undeclared = Some(argument);
        }
    }
    let work = WorkCounts::default();
    let status = match undeclared {
        Some(argument) => {
            eprintln!("mara: unknown argument `{}`", argument.to_string_lossy());
            ExitCode::from(2)
        }
        None => ExitCode::SUCCESS,
    };
    if debug_stats {
        eprintln!("{}", debug_stats::line(&work));
    }
    status
}
