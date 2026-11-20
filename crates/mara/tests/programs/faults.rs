//! VER-PROGRAM-FAULTS: each faulty program is consumed by an invariant of each severity, a
//! query and a gate that names two items. `project validate` reports the fault and is invalid,
//! `query execute` fails with it, the gate and both items are unavailable, and so is `query
//! report` of the invariant, with no row and no explanation.

use crate::{
    GATE, PROGRAMS_LINE, QUERY, Sandbox, assert_fault, assert_unavailable, both_severities,
};

/// The compile errors of EVD-SOUFFLE-TRIAL: each program declares `bad(m: Mid)` and has the
/// error at the given line.
const COMPILE_ERRORS: [(&str, &str, u64); 5] = [
    (
        "undefined relation",
        ".decl bad(m: Mid)\nbad(r) :- requirment(r).\n",
        2,
    ),
    (
        "wrong arity",
        ".decl bad(m: Mid)\nbad(r) :- edge(\"verifies\", _, r).\n",
        2,
    ),
    (
        "a Mid used as an Id",
        ".decl ids(i: Id)\nids(m) :- item(m, _, _, _, _, _).\n.decl bad(m: Mid)\nbad(m) :- ids(m).\n",
        2,
    ),
    (
        "a symbol used as a number",
        ".decl score(r: Mid, s: number)\nscore(r, l) :- field(r, \"owner\", l).\n\
         .decl bad(m: Mid)\nbad(m) :- score(m, _).\n",
        2,
    ),
    (
        "negation that cannot be stratified",
        ".decl bad(x: Mid)\n.decl q(x: Mid)\nbad(x) :- item(x, _, _, _, _, _), !q(x).\n\
         q(x) :- item(x, _, _, _, _, _), !bad(x).\n",
        0,
    ),
];

/// Requires an error in `path` at `line`, or anywhere in it for 0, whose message ends with the
/// source line, then a caret under a column.
#[track_caller]
fn assert_excerpt(run: &crate::Run, path: &str, source: &str, line: u64) {
    let found = run.with_code("program-error");
    let found = found
        .iter()
        .find(|d| d["path"] == path && (line == 0 || d["line"] == line));
    let Some(found) = found else {
        panic!("no program-error in {path}: {:#?}", run.diagnostics());
    };
    let message = found["message"].as_str().unwrap();
    let lines: Vec<&str> = message.lines().collect();
    let [.., text, caret] = lines[..] else {
        panic!("{message:?}")
    };
    let number = found["line"].as_u64().unwrap();
    assert_eq!(
        text,
        source.lines().nth(number as usize - 1).unwrap(),
        "{message}"
    );
    let column = caret.len() - 1;
    assert!(caret.ends_with('^') && caret[..column].trim().is_empty() && column < text.len());
    assert_eq!(
        run.with_code("as-error").len(),
        0,
        "a faulty program has no rows"
    );
}

#[test]
fn compile_errors_are_reported_at_their_line_with_a_caret() {
    for (case, body, line) in COMPILE_ERRORS {
        let sandbox = Sandbox::new();
        let program = format!(".include \"mara.dl\"\n{body}");
        sandbox.program("p.dl", &program, &both_severities("p.dl", "bad"));
        let run = sandbox.validate();
        assert_eq!(run.status, 1, "{case}: {run:#?}");
        let line = line + u64::from(line > 0);
        assert_excerpt(&run, ".mara/rules/p.dl", &program, line);
        assert_eq!(run.with_code("as-warning").len(), 0);
        assert_unavailable(&sandbox, "program-error", ".mara/rules/p.dl", line, &["^"]);
    }
}

/// The same errors in an included file, which shares its base name with another included file:
/// Soufflé names both `q.dl`.
#[test]
fn compile_errors_in_an_included_file_are_reported_in_that_file() {
    for (case, body, line) in COMPILE_ERRORS {
        let sandbox = Sandbox::new();
        let main = ".include \"mara.dl\"\n.include \"a/q.dl\"\n.include \"b/q.dl\"\n";
        sandbox.program("main.dl", main, &both_severities("main.dl", "bad"));
        sandbox.write(
            ".mara/rules/a/q.dl",
            ".once\n.decl fine(x: number)\nfine(1).\n",
        );
        let included = format!(".once\n{body}");
        sandbox.write(".mara/rules/b/q.dl", &included);
        let run = sandbox.validate();
        assert_eq!(run.status, 1, "{case}: {run:#?}");
        let line = line + u64::from(line > 0);
        assert_excerpt(&run, ".mara/rules/b/q.dl", &included, line);
        assert_unavailable(
            &sandbox,
            "program-error",
            ".mara/rules/b/q.dl",
            line,
            &["^"],
        );
        assert!(
            run.with_code("program-error")
                .iter()
                .all(|d| d["path"] == ".mara/rules/b/q.dl")
        );
    }
}

#[test]
fn a_refused_program_is_reported_where_the_parser_stopped_and_never_runs() {
    let sandbox = Sandbox::new();
    let program = ".include \"mara.dl\"\n.decl bad(m: Mid)\nbad(m) :- item(m, _, _, _, _, _), m = as(1, Mid).\n";
    sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
    let run = sandbox.validate();
    assert_fault(&run, "program-refused", ".mara/rules/p.dl", 3, &["^"]);
    assert_eq!((run.spawns(), run.runs()), (0, 0));
    for run in assert_unavailable(&sandbox, "program-refused", ".mara/rules/p.dl", 3, &["^"]) {
        assert_eq!((run.spawns(), run.runs()), (0, 0), "{run:#?}");
    }
}

#[test]
fn to_number_of_text_is_a_runtime_fault_at_its_rule() {
    let sandbox = Sandbox::new();
    let program = ".include \"mara.dl\"\n.decl bad(m: Mid)\n\
                   bad(m) :- field(m, \"owner\", v), to_number(v) > 0.\n";
    sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
    let run = sandbox.validate();
    assert_fault(
        &run,
        "program-fault",
        ".mara/rules/p.dl",
        3,
        &["to_number(\"abc\")"],
    );
    assert_eq!(run.with_code("as-error").len(), 0);
    let parts = ["to_number(\"abc\")"];
    assert_unavailable(&sandbox, "program-fault", ".mara/rules/p.dl", 3, &parts);
}

#[test]
fn a_warning_while_evaluating_is_a_fault_although_soufflé_exits_with_0() {
    let sandbox = Sandbox::new();
    let program = ".include \"mara.dl\"\n.decl bad(m: Mid)\n\
                   bad(m) :- title(m, t), substr(t, 100, 1) = \"x\".\n";
    sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
    let run = sandbox.validate();
    assert_fault(
        &run,
        "program-fault",
        ".mara/config.toml",
        PROGRAMS_LINE,
        &["substr"],
    );
    let config = ".mara/config.toml";
    assert_unavailable(
        &sandbox,
        "program-fault",
        config,
        PROGRAMS_LINE,
        &["substr"],
    );
}

#[test]
fn an_output_that_is_not_utf8_is_a_fault() {
    let sandbox = Sandbox::new();
    // `é` of the title `Héllo` takes bytes 1 and 2.
    let program = ".include \"mara.dl\"\n.decl bad(m: Mid, s: symbol)\n\
                   bad(m, substr(t, 0, 2)) :- title(m, t).\n";
    sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
    let run = sandbox.validate();
    assert_fault(
        &run,
        "program-fault",
        ".mara/config.toml",
        PROGRAMS_LINE,
        &["UTF-8"],
    );
    assert_eq!(run.with_code("as-error").len(), 0);
    let config = ".mara/config.toml";
    assert_unavailable(&sandbox, "program-fault", config, PROGRAMS_LINE, &["UTF-8"]);
}

#[test]
fn lenient_text_functions_are_no_fault() {
    let sandbox = Sandbox::new();
    let program = ".include \"mara.dl\"\n.decl bad(m: Mid)\n\
                   bad(m) :- item(m, _, _, \"accepted\", _, _), to_number(\"7junk\") = 7, \
                   substr(\"abcdef\", 2, -1) = \"cdef\".\n";
    sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
    let run = sandbox.validate();
    assert_eq!(run.status, 1, "{run:#?}");
    assert_eq!(run.with_code("as-error").len(), 2, "{run:#?}");
    assert_eq!(run.with_code("as-warning").len(), 2);
    assert_eq!(run.diagnostics().len(), 4);
    let query = sandbox.mara(&QUERY);
    assert_eq!(query.status, 0, "{query:#?}");
    assert_eq!(query.json["rows"].as_array().unwrap().len(), 2);
    let gate = sandbox.mara(&GATE);
    assert_eq!(gate.json["gate"]["outcome"], "failed", "{gate:#?}");
}

#[test]
fn an_output_past_the_size_limit_is_a_fault_without_rows() {
    let sandbox = Sandbox::new();
    // Two accepted items × 140,000 rows of about 1 kB: at least twice the limit of 64 MiB.
    let text = "x".repeat(1000);
    let program = format!(
        ".include \"mara.dl\"\n.decl n(x: number)\nn(x) :- x = range(0, 140000).\n\
         .decl bad(m: Mid, x: number, s: symbol)\n\
         bad(m, x, \"{text}\") :- item(m, _, _, \"accepted\", _, _), n(x).\n"
    );
    sandbox.program("p.dl", &program, &both_severities("p.dl", "bad"));
    let run = sandbox.validate();
    assert_fault(
        &run,
        "program-fault",
        ".mara/config.toml",
        PROGRAMS_LINE,
        &["output-size limit of 64 MiB"],
    );
    assert_eq!(run.with_code("as-error").len(), 0);
    let parts = ["output-size limit of 64 MiB"];
    assert_unavailable(
        &sandbox,
        "program-fault",
        ".mara/config.toml",
        PROGRAMS_LINE,
        &parts,
    );
    assert_eq!(sandbox.tmp_entries(), Vec::<String>::new());
}

#[test]
fn a_program_past_the_memory_limit_is_stopped_before_the_time_limit() {
    let sandbox = Sandbox::new();
    // 10^10 pairs: far more than twice the limit of 1 GiB.
    let program = ".include \"mara.dl\"\n.decl n(x: number)\nn(x) :- x = range(0, 100000).\n\
                   .decl p(x: number, y: number)\np(x, y) :- n(x), n(y).\n\
                   .decl bad(m: Mid, c: number)\n\
                   bad(m, c) :- item(m, _, _, _, _, _), c = count : p(_, _).\n";
    sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
    let run = sandbox.validate();
    assert_fault(
        &run,
        "program-fault",
        ".mara/config.toml",
        PROGRAMS_LINE,
        &["memory limit of 1024 MiB"],
    );
    assert!(run.elapsed.as_secs() < 10, "{:?}", run.elapsed);
    let parts = ["memory limit of 1024 MiB"];
    let config = ".mara/config.toml";
    for run in assert_unavailable(&sandbox, "program-fault", config, PROGRAMS_LINE, &parts) {
        assert!(run.elapsed.as_secs() < 10, "{:?}", run.elapsed);
    }
    assert_eq!(sandbox.tmp_entries(), Vec::<String>::new());
}

/// An `.include` earlier on the line has another path in the private copy; the caret stays
/// under the error in the project file.
#[test]
fn the_caret_is_placed_after_an_include_on_the_same_line() {
    let sandbox = Sandbox::new();
    let program = ".include \"verylongfilename.dl\" .decl bad(m: symbol) bad(m) :- missing(m).\n";
    sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
    sandbox.write(
        ".mara/rules/verylongfilename.dl",
        ".once\n.decl other(x: number)\nother(1).\n",
    );
    let run = sandbox.validate();
    let errors = run.with_code("program-error");
    assert_eq!(errors.len(), 1, "{run:#?}");
    let message = errors[0]["message"].as_str().unwrap();
    let caret = message.lines().last().unwrap();
    assert_eq!(
        caret.len() - 1,
        program.find("missing").unwrap(),
        "{message}"
    );
}

/// An inherited hard core limit of 0, as under a service with `LimitCORE=0`, changes no result:
/// the host never raises it to give Soufflé its core limit (DES-RULE-EXECUTION).
#[cfg(target_os = "linux")]
#[test]
fn a_hard_core_limit_of_0_changes_no_result() {
    use std::os::unix::process::CommandExt;
    let sandbox = Sandbox::new();
    let program = ".include \"mara.dl\"\n.decl bad(m: Mid)\n\
                   bad(m) :- item(m, _, _, \"accepted\", _, _).\n";
    sandbox.program("p.dl", program, &both_severities("p.dl", "bad"));
    let args = ["project", "validate", "--format", "json"];
    let normal = sandbox.command(&args).output().expect("start mara");
    let mut command = sandbox.command(&args);
    // SAFETY: the closure runs in the child before `exec` and calls only `setrlimit`, which is
    // async-signal-safe.
    unsafe {
        command.pre_exec(|| {
            let zero = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::setrlimit(libc::RLIMIT_CORE, &zero) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let limited = command.output().expect("start mara");
    let json: serde_json::Value = serde_json::from_slice(&limited.stdout).unwrap();
    let codes: Vec<&str> = json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|diagnostic| diagnostic["code"].as_str().unwrap())
        .collect();
    assert_eq!(
        codes,
        ["as-error", "as-warning", "as-error", "as-warning"],
        "{json:#}"
    );
    assert_eq!(limited.status.code(), normal.status.code());
    assert_eq!(
        String::from_utf8_lossy(&limited.stdout),
        String::from_utf8_lossy(&normal.stdout)
    );
}
