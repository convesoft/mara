//! The Node and browser cases of the host verifications (VER-HOST-COMPARISON in
//! `docs/verification.mara.md`): ES modules in `node/`, run by `node --test`, Node's own runner,
//! which drive Playwright's Chromium through Playwright's library and compare each host with CLI
//! direct mode. Each test here prepares the fixtures of one module and runs it ([`node`]).
//!
//! | Module | Cases |
//! | --- | --- |
//! | `node/browser/workspace.test.mjs` | VER-BROWSER-WORKSPACE |
//! | `node/browser/unavailable.test.mjs` | The browser workspace's part of VER-UNAVAILABLE-NOT-PARTIAL |
//! | `node/browser/work_counters.test.mjs` | The browser workspace's warm rows of VER-WORK-COUNTERS |
//! | `node/browser/corpus.test.mjs` | The 1× corpus row of VER-HOST-COMPARISON: the `engineering` template's queries, views, gate and report |
//! | `node/browser/programs.test.mjs` | The browser rows of VER-FACT-ROWS, VER-EXPLANATIONS, VER-PROGRAM-FAULTS and VER-PROGRAM-REACH-CASES |
//! | `node/browser/limits.test.mjs` | The browser row of VER-EVALUATION-TIME-LIMIT, and a module that aborts |
//! | `node/library/parity.test.mjs` | The first three rows of the Node and browser table of VER-HOST-COMPARISON, the parity cases of VER-NODE-LIBRARY, the Chromium case of VER-BROWSER-ACCESS and the daemon part of the first row of VER-UNAVAILABLE-NOT-PARTIAL |
//! | `node/library/types.test.mjs` | The `tsc` case of VER-NODE-LIBRARY |
//! | `node/library/timing.test.mjs` | The time-limit case of VER-NODE-LIBRARY |
//! | `node/library/daemon.test.mjs` | The Node rows of VER-DAEMON-START, VER-DAEMON-WRITES and VER-CONCURRENT-WRITERS |

// The setup ends leftover processes by signal.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../support/corpus.rs"]
mod corpus;
mod fixtures;
#[path = "../support/legacy_project.rs"]
mod legacy_project;
mod node;

use node::Host;

/// The seed of the performance harness's corpora (DES-PERFORMANCE-HARNESS).
const SEED: u64 = 0x4D41_5241;

#[test]
fn browser_workspace() {
    let host = Host::new();
    fixtures::workspace(&host);
    host.run("browser/workspace.test.mjs");
}

#[test]
fn browser_unavailable() {
    let host = Host::new();
    fixtures::workspace(&host);
    fixtures::grammars(&host);
    fixtures::legacy(&host);
    fixtures::programs(&host);
    host.run("browser/unavailable.test.mjs");
}

#[test]
fn browser_work_counters() {
    let host = Host::new();
    let corpus = corpus::Corpus::generate(1, SEED);
    fixtures::copy(corpus.root(), &host.fixture("corpus"));
    // The document of the first cluster, which the caller writes.
    let described = serde_json::json!({ "document": corpus.ids[0].1, "id": corpus.first_id });
    std::fs::write(host.fixture("corpus.json"), described.to_string()).unwrap();
    host.run("browser/work_counters.test.mjs");
}

#[test]
fn browser_programs() {
    let host = Host::new();
    fixtures::programs(&host);
    fixtures::facts(&host);
    host.run("browser/programs.test.mjs");
}

#[test]
fn browser_limits() {
    let host = Host::new();
    fixtures::programs(&host);
    host.run("browser/limits.test.mjs");
}

#[test]
fn browser_corpus_programs() {
    let host = Host::new();
    let corpus = corpus::Corpus::generate(1, SEED);
    fixtures::copy(corpus.root(), &host.fixture("corpus"));
    let described = serde_json::json!({
        "gate_requirement": corpus.gate_requirement,
        "retired_requirement": corpus.retired_requirement,
        "candidate": corpus::CANDIDATE,
        "results": corpus::RESULTS,
    });
    std::fs::write(host.fixture("corpus.json"), described.to_string()).unwrap();
    host.run("browser/corpus.test.mjs");
}

#[test]
fn library_parity() {
    let host = Host::new();
    let corpus = corpus::Corpus::generate(1, SEED);
    fixtures::copy(corpus.root(), &host.fixture("corpus"));
    fixtures::legacy(&host);
    host.run("library/parity.test.mjs");
}

#[test]
fn library_types() {
    let host = Host::new();
    host.run("library/types.test.mjs");
}

#[test]
fn library_timing() {
    let host = Host::new();
    fixtures::programs(&host);
    host.run("library/timing.test.mjs");
}

#[test]
fn library_daemon() {
    let host = Host::new();
    fixtures::workspace(&host);
    host.run("library/daemon.test.mjs");
}
