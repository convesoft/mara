//! The invariant cases of VER-EVALUATION-TIME-LIMIT.

use std::fs;
use std::time::Duration;

use crate::{PROGRAMS_LINE, Sandbox, assert_fault, both_severities, config};

/// A `count` over the four-fold product of a `range` of 1,000 numbers: hours, little memory,
/// one output row per item. Soufflé keeps the aggregate's body as a relation of its own, so a
/// condition that no combination meets keeps that relation empty.
pub const LONG: &str = ".include \"mara.dl\"\n.decl n(x: number)\nn(x) :- x = range(0, 1000).\n\
                    .decl bad(m: Mid, c: number)\n\
                    bad(m, c) :- item(m, _, _, _, _, _), \
                    c = count : { n(w), n(x), n(y), n(z), w + x + y + z < 0 }.\n";

pub fn with_limit(sandbox: &Sandbox, program: &str, seconds: Option<u32>) {
    let entries = both_severities("p.dl", "bad");
    sandbox.program("p.dl", program, &entries);
    if let Some(seconds) = seconds {
        let text = config(&entries).replace(
            "programs = [\".mara/rules/**/*.dl\"]\n",
            &format!("programs = [\".mara/rules/**/*.dl\"]\ntime_limit_seconds = {seconds}\n"),
        );
        sandbox.write(".mara/config.toml", &text);
    }
}

/// No `souffle` process has its working directory under the sandbox.
fn assert_no_souffle_left(sandbox: &Sandbox) {
    let root = sandbox.root();
    for entry in fs::read_dir("/proc").unwrap().flatten() {
        let Ok(cwd) = fs::read_link(entry.path().join("cwd")) else {
            continue;
        };
        assert!(
            !cwd.starts_with(&root),
            "a process runs in {}",
            cwd.display()
        );
    }
}

#[test]
fn a_configured_limit_stops_the_evaluation() {
    let sandbox = Sandbox::new();
    with_limit(&sandbox, LONG, Some(1));
    let before = sandbox.tmp_entries();
    let run = sandbox.validate();
    assert_fault(
        &run,
        "program-fault",
        ".mara/config.toml",
        PROGRAMS_LINE,
        &["time limit of 1 s"],
    );
    assert!(run.elapsed >= Duration::from_secs(1), "{:?}", run.elapsed);
    assert!(run.elapsed < Duration::from_secs(10), "{:?}", run.elapsed);
    assert_no_souffle_left(&sandbox);
    assert_eq!(sandbox.tmp_entries(), before);
}

#[test]
fn the_default_limit_is_10_s() {
    let sandbox = Sandbox::new();
    with_limit(&sandbox, LONG, None);
    let run = sandbox.validate();
    assert_fault(
        &run,
        "program-fault",
        ".mara/config.toml",
        PROGRAMS_LINE,
        &["time limit of 10 s"],
    );
    assert!(run.elapsed >= Duration::from_secs(10), "{:?}", run.elapsed);
    assert!(run.elapsed < Duration::from_secs(20), "{:?}", run.elapsed);
    assert_no_souffle_left(&sandbox);
}

/// The two programs of EVD-SOUFFLE-PROGRAM-REACH that never end.
#[test]
fn programs_that_never_end_are_stopped_by_a_limit() {
    let programs = [
        ".include \"mara.dl\"\n.decl nat(x: number)\nnat(0).\nnat(x + 1) :- nat(x).\n\
         .decl bad(m: Mid)\nbad(m) :- item(m, _, _, _, _, _), nat(-1).\n",
        ".include \"mara.dl\"\n.decl s(x: symbol)\ns(\"a\").\ns(cat(x, \"a\")) :- s(x).\n\
         .decl bad(m: Mid)\nbad(m) :- item(m, _, _, _, _, _), s(\"b\").\n",
    ];
    for program in programs {
        let sandbox = Sandbox::new();
        with_limit(&sandbox, program, Some(1));
        let run = sandbox.validate();
        let fault = run.with_code("program-fault");
        let message = fault.first().map_or("", |d| d["message"].as_str().unwrap());
        assert!(
            message.contains("time limit of 1 s") || message.contains("memory limit"),
            "{run:#?}"
        );
        assert_fault(
            &run,
            "program-fault",
            ".mara/config.toml",
            PROGRAMS_LINE,
            &["limit"],
        );
        assert!(run.elapsed < Duration::from_secs(10), "{:?}", run.elapsed);
        assert_no_souffle_left(&sandbox);
        assert_eq!(sandbox.tmp_entries(), Vec::<String>::new());
    }
}
