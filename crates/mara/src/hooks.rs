//! The hooks that tests set through environment variables, in debug builds only
//! (ADR-DEBUG-PAUSE-POINT in `docs/authoring/writes.mara.md`). Every hook reads its variable
//! through [`value`], the one place that names the variables and the one `cfg(debug_assertions)`
//! boundary: a release build reads none of them and holds none of their names. Each function
//! takes the hook and acts only on what [`value`] gives, so in a release build none does anything.
//! Public only for the binary crate, which reads [`path`] and [`number`] and pauses at [`pause`].

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// A hook, by the variable that sets it.
#[derive(Clone, Copy, Debug)]
pub enum Hook {
    /// `MARA_TEST_PAUSE`: the writer's pause points (DES-WRITE-PROCEDURE).
    Pause,
    /// `MARA_TEST_PAUSE_EVALUATION`: a rule evaluation's pause point.
    PauseEvaluation,
    /// `MARA_TEST_PAUSE_WATCH`: the watcher's pause point.
    PauseWatch,
    /// `MARA_TEST_DERIVED`: the report of the daemon's background derivations.
    Derived,
    /// `MARA_TEST_IDLE_SECONDS`: the daemon's idle period (ADR-TEST-IDLE-PERIOD).
    IdleSeconds,
    /// `MARA_TEST_GRAMMAR_LOAD_MS`: the time a grammar may take to load.
    GrammarLoadMs,
    /// `MARA_TEST_GRAMMAR_BUDGET_MS`: the time a grammar may take on a file.
    GrammarBudgetMs,
    /// `MARA_TEST_GRAMMAR_STALL`: a file that no grammar finishes parsing.
    GrammarStall,
    /// `MARA_TEST_GRAMMAR_LOAD_STALL`: a grammar whose load is held.
    GrammarLoadStall,
    /// `MARA_TEST_GRAMMAR_EVENTS`: the directory of the grammar events and the hold.
    GrammarEvents,
    /// `MARA_TEST_GRAMMAR_CATALOG`: the catalog that `code add` reads instead of its own.
    GrammarCatalog,
    /// `MARA_TEST_PAUSE_ANSWER`: the daemon's listener for browsers before it sends an
    /// operation's answer.
    PauseAnswer,
}

/// The value of `hook`'s variable when it is set and not empty; in a release build, never.
#[cfg(debug_assertions)]
pub(crate) fn value(hook: Hook) -> Option<OsString> {
    let name = match hook {
        Hook::Pause => "MARA_TEST_PAUSE",
        Hook::PauseEvaluation => "MARA_TEST_PAUSE_EVALUATION",
        Hook::PauseWatch => "MARA_TEST_PAUSE_WATCH",
        Hook::Derived => "MARA_TEST_DERIVED",
        Hook::IdleSeconds => "MARA_TEST_IDLE_SECONDS",
        Hook::GrammarLoadMs => "MARA_TEST_GRAMMAR_LOAD_MS",
        Hook::GrammarBudgetMs => "MARA_TEST_GRAMMAR_BUDGET_MS",
        Hook::GrammarStall => "MARA_TEST_GRAMMAR_STALL",
        Hook::GrammarLoadStall => "MARA_TEST_GRAMMAR_LOAD_STALL",
        Hook::GrammarEvents => "MARA_TEST_GRAMMAR_EVENTS",
        Hook::GrammarCatalog => "MARA_TEST_GRAMMAR_CATALOG",
        Hook::PauseAnswer => "MARA_TEST_PAUSE_ANSWER",
    };
    std::env::var_os(name).filter(|value| !value.is_empty())
}

#[cfg(not(debug_assertions))]
#[inline(always)]
pub(crate) fn value(_: Hook) -> Option<OsString> {
    None
}

/// The file or directory that `hook`'s variable names.
pub fn path(hook: Hook) -> Option<PathBuf> {
    value(hook).map(PathBuf::from)
}

/// The whole number that `hook`'s variable holds.
pub fn number(hook: Hook) -> Option<u64> {
    value(hook)?.to_str()?.parse().ok()
}

/// A pause point: when `hook`'s variable names a directory, creates `paused` there, whole and
/// holding `contents`, and waits until it is gone.
pub fn pause(hook: Hook, contents: &str) -> io::Result<()> {
    static STAGED: AtomicU64 = AtomicU64::new(0);
    let Some(directory) = path(hook) else {
        return Ok(());
    };
    let paused = directory.join("paused");
    let staged = directory.join(format!(
        ".paused-{}-{}",
        std::process::id(),
        STAGED.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&staged, contents)?;
    fs::rename(&staged, &paused)?;
    while fs::symlink_metadata(&paused).is_ok() {
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

/// Appends `line` and a newline, from one buffer, to the file of `hook`'s variable: the file
/// that `MARA_TEST_DERIVED` names, or `events` in the directory that `MARA_TEST_GRAMMAR_EVENTS`
/// names. A failure is ignored.
pub(crate) fn append(hook: Hook, line: fmt::Arguments) {
    let Some(mut file) = path(hook) else {
        return;
    };
    if let Hook::GrammarEvents = hook {
        file.push("events");
    }
    let opened = fs::OpenOptions::new().create(true).append(true).open(file);
    let _ = opened.and_then(|mut opened| opened.write_all(format!("{line}\n").as_bytes()));
}
