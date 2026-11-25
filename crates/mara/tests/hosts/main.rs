//! The Node and browser cases of the host verifications (VER-HOST-COMPARISON in
//! `docs/verification.mara.md`): ES modules in `node/`, run by `node --test`, Node's own runner,
//! which drive Playwright's Chromium through Playwright's library and compare each host with CLI
//! direct mode. Each test here prepares the fixtures of one module and runs it ([`node`]).
//!
//! | Module | Cases |
//! | --- | --- |
//! | `node/browser/workspace.test.mjs` | VER-BROWSER-WORKSPACE |
//! | `node/browser/unavailable.test.mjs` | The browser workspace's part of the first two rows of VER-UNAVAILABLE-NOT-PARTIAL |
//! | `node/browser/work_counters.test.mjs` | The browser workspace's warm rows of VER-WORK-COUNTERS |

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
