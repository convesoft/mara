# Rust verification

This repository workflow implements the user-requested impact-based development
and review loop. Targeted execution is not full regression coverage. CI remains
authoritative for integration; installed Node/browser and supported-host gates
in `.github/workflows/ci.yml` and `release.yml` remain required where applicable.

## Tools and scope

Use Cargo, [nextest](https://nexte.st/) 0.9.148, and
[cargo-affected](https://github.com/max-sixty/cargo-affected) pinned to
`6955352192882134d0fb0df302e849fff27ab234` (upstream PR #54). The user selected
this experimental upstream fix after investigation found that released 0.4.1
ignores input-rule filters, selecting the entire workspace. Do not substitute
a registry release based on its version string; the pinned build reports 0.4.0.

```sh
cargo install cargo-nextest --version 0.9.148 --locked
cargo install cargo-affected --git https://github.com/max-sixty/cargo-affected \
  --rev 6955352192882134d0fb0df302e849fff27ab234 --locked
rustup component add llvm-tools
```

The workspace contains native `mara`, portable `mara-engine`, and the
`mara-wasm`/`mara-node` binding crates. Tests include library units, integration
binaries, CLI/MCP subprocesses, and explicitly ignored acceptance/subprocess
helpers. Nextest runs each non-ignored test separately. Keep real subprocess
helpers and external acceptance commands; do not use `--run-ignored all`.
Doctests still use Cargo because nextest does not run them.

Upstream is actively changing but explicitly warns that it is extremely early,
with no support or cache-schema migration promise. Linux and macOS are supported
by upstream; its Windows support does not extend Mara's platform contract.
Impact analysis uses LLVM function ranges and old-side Git diff hunks, not a
static dependency guarantee. No custom impact engine is maintained here.

## Storage

`scripts/verify.sh` requires existing absolute `CARGO_TARGET_DIR`, `TMPDIR`,
`MARA_TEST_RUNTIME_DIR`, and an absolute `VERIFY_EVIDENCE_DIR` outside the checkout.
Reuse the build/cache directory across local checks. Use disk-backed storage
with enough space: native and instrumented builds are separate and sizeable.
The IPC directory must be private and short enough for Unix sockets.

Humans can use the ordinary Cargo target and retained user-state evidence:

```sh
export CARGO_TARGET_DIR="$PWD/target" TMPDIR="$PWD/target/tmp"
export VERIFY_EVIDENCE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/mara/verification"
export MARA_TEST_RUNTIME_DIR=$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/mv.XXXXXX")
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR" "$VERIFY_EVIDENCE_DIR"
# Remove only this owned IPC directory after the checks finish.
```

Agents instead follow their session-owned Pi scratch/evidence policy. Do not
reuse the developer's target or another session's cache. For this tool, the script
routes upstream's hardcoded `target/affected` and nextest's metadata store
`target/nextest` through temporary symlinks into the corresponding
`$CARGO_TARGET_DIR` directories, then removes its own links on exit. It refuses to
replace an existing different cache. Use a disposable checkout if that cache
already belongs to someone else. Do not run simultaneous refreshes/runs sharing
a coverage database, edit source during verification, or put evidence inside it.

## Commands

```sh
scripts/verify.sh impacted                       # normal development loop
scripts/verify.sh selected 'binary(=item_update)' # explicit nextest filter
scripts/verify.sh selected 'test(/stale_cursor/)' # individual names/substrings
scripts/verify.sh refresh                        # complete coverage collection
scripts/verify.sh refresh-diff                   # incremental coverage refresh
scripts/verify.sh full                           # integration verification
scripts/verify.sh check /absolute/receipt        # check evidence before reuse
```

### Targeted development

Collect against a clean committed baseline before starting a slice. Never use
`cargo affected collect --allow-dirty`: its line numbers would not describe its
recorded SHA. `impacted` compares the current working tree, including committed
changes since collection, against each test's recorded collection SHA. An older
baseline intentionally broadens selection. There is no arbitrary `main` default
or silently substituted merge base. `refresh-diff` reanchors only rerun tests;
commit first. New non-ignored tests absent from the database are included.

Coverage excludes binding crates because upstream cannot map their `cdylib` /
`rlib` test metadata. `impacted` always runs all binding tests explicitly via
nextest, in addition to coverage-selected native/engine tests. `refresh` refreshes
native/engine coverage, not full verification or binding coverage.

`Cargo.toml` input rules force native integration tests for native/engine source
and shared test-helper changes: LLVM's test-binary map does not reliably include
code executed in our separate CLI process. Template/fixture changes select all
covered tests. Build/test configuration selects broadly and invalidates the
script's context stamp. Ordinary test edits can remain narrowly selected.
Documentation-only edits need their applicable documentation checks, not an
automatic complete Rust suite.

The context stamp also includes tool binary identities, compiler/OS, manifests,
lockfile, build scripts, Cargo config (ancestor/user included), nextest config,
toolchain files and relevant environment-variable hashes. Missing or changed
context forces full native/engine selection plus binding tests; it never silently
uses stale ranges. A changed context blocks `refresh-diff`; use `refresh`.
Upstream additionally broadens for a missing database/fingerprint/collection SHA
and includes tests without usable coverage. A daemon termination can leave a
corrupt LLVM profile; observed unmapped tests are reselected rather than skipped.
Tool errors fail verification; they
are not interpreted as zero impacted tests. The selected build is locked,
workspace/default-features/all-targets; explicitly verify other feature/target
configurations when the task requires them.

### Review evidence

Every execution prints a unique receipt directory with the original revision,
working-tree status and content hashes (including untracked/deleted files),
selection/filter, exact commands, environment/config hashes, output, exit status,
and JUnit where available (`native-junit.xml` for the impacted native phase,
`junit.xml` for its binding phase). `selection.json` describes selection, not a passing
run. Only an unchanged successful invocation gets `passed`.

Reviewers inspect scope, failures/skips, and existing receipts first. `check`
rejects incomplete/failed receipts, changed inputs or changed recorded execution
context. Matching evidence can be reused for its recorded scope; it is not renamed
to a new tested SHA. Storage-only directory changes are not semantic environment
changes. The strict content snapshot may reject harmless Work/docs-only edits;
reviewers may establish and record unchanged tested inputs manually rather than
rerunning identical tests. A reviewer can request additional selected or broader
checks for a concrete gap. Do not repeatedly run the full suite without a reason.

External state is not exhaustively fingerprinted: changes to external executables,
services, mutable files at environment-supplied paths or unrecorded build inputs
require explicit invalidation (`cargo affected clean`, then `refresh`) or broader
execution. Proc-macro changes, new non-Rust runtime inputs and uncertain impact
require broader tests and an input rule where appropriate. Incremental refresh
can overselect due to upstream issue #94; complete refresh resets anchors.

### Full integration gate

`full` runs formatting, workspace/all-target build, strict all-feature Clippy,
every non-ignored workspace/all-target test through nextest (ignoring any profile
default filter), Cargo doctests, and
complete schema/project validation. No impact selection/exclusions apply.
Required ignored acceptance probes, WASM builds, installed-artifact/browser tests
and supported-host checks remain their existing CI/release gates, not implied by
local Rust success. CI performs full verification, never impact-selected tests.
Evidence reuse does not waive integration checks on the actual combined inputs.
