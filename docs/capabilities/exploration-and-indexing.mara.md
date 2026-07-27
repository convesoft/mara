# Exploration and indexing

Mara's first read interface is a CLI over the current working tree. It supports
direct inspection without a database and can produce a stable JSON projection
for later derived interfaces.

## Common query behaviour

:::req m_01KY7YA2CNMH2B11VJN1V9RCQY
:id: REQ-QUERY-RESOLUTION
:title: Query commands shall resolve MID or display ID
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: SCN-EXPLORE-ITEM

Any command that accepts an item reference shall apply the same exact MID-then-
display-ID resolution contract as validation and shall report unresolved or
ambiguous input without heuristic matching.
:::

:::req m_01KY7YA2CPKFBF7ZHDMSPD7VNB
:id: REQ-QUERY-FORMATS
:title: Query commands shall provide human and JSON output
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-NAVIGATE-TRACE
:derives_from: GOAL-AGENT-READY

`list`, `show`, and `trace` shall support `--format human|json`. Human output
shall optimize terminal reading; JSON output shall expose complete stable data
without scraping terminal presentation.
:::

## Item listing

:::req m_01KY7YA2CQ5ZDY4TPSF0Z0RC3S
:id: REQ-LIST-ITEMS
:title: mara list shall enumerate project items deterministically
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-NAVIGATE-TRACE

`mara list` shall enumerate items in a stable order and show at least MID,
display ID when present, flavour, title, and project-relative source location.
:::

:::req m_01KY7YA2CR1DSJYV2X25F9PMF4
:id: REQ-LIST-FILTERS
:title: mara list shall filter by flavour and field values
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: STORY-NAVIGATE-TRACE

The list command shall accept repeated flavour filters and exact scalar field
filters. Values within one flavour or field filter shall combine with OR;
different field names and the flavour constraint shall combine with AND. The
JSON result shall record normalized effective filters even when they are empty.
A repeatable field shall match when any authored value equals any compiled filter
value for the item's flavour; absent fields shall not match.
:::

## Item inspection

:::req m_01KY7YA2CSXP38N88WWVN65SN9
:id: REQ-SHOW-ITEM
:title: mara show shall expose the complete resolved item
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-EXPLORE-ITEM

`mara show <ref>` shall display structural identity, built-ins, typed fields,
body, document placement, source spans, authored relation occurrences,
canonical outgoing and incoming relations, weak mentions, backlinks, and
per-occurrence provenance.
:::

## Trace traversal

:::req m_01KY7YA2CT9Z63V0KZR746BG4Z
:id: REQ-TRACE-DIRECTION
:title: mara trace shall traverse selected edge directions
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-EXPLORE-ITEM

`mara trace <ref>` shall support incoming, outgoing, and bidirectional traversal
over canonical typed edges, with explicit identification of the direction and
relation used for each result.
:::

:::req m_01KY7YA2CVQYV5XFGPPD1JVC9C
:id: REQ-TRACE-DEPTH
:title: Trace traversal shall be bounded and cycle-safe
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-NAVIGATE-TRACE

Trace shall default to direct neighbours and accept a bounded positive depth.
It shall return every distinct simple canonical-edge path from the focus with one
through the selected maximum number of steps, never repeat an item, derived
source-span, or external-node identity within a path, exclude the zero-step focus
path, and order nodes and paths deterministically. Different canonical relation
names between the same endpoints are distinct paths; repeated authored
occurrences of one canonical edge are not.
:::

## Deterministic JSON projection

:::req m_01KY7YA2CWWZ6K78Q7ERE0F0S5
:id: REQ-INDEX-COMMAND
:title: mara index shall write a rebuildable JSON projection
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-GIT-CANONICAL
:uses_term: TERM-DERIVED-PROJECTION

`mara index` shall validate the project and write the configured JSON index only
when a coherent normalized model exists and no diagnostic fails the configured
validation policy. Warning escalation therefore suppresses the write. Non-failing
warning and information diagnostics may be included in the projection. The index
shall be a replaceable generated file and never an authoring input. A successful
write shall atomically replace the configured index with one complete canonical
document; validation or write failure shall never expose a truncated or partially
serialized index.
:::

:::req m_01KY7YA2CX018G0FHR43XYW4XM
:id: REQ-INDEX-CONTENT
:title: The JSON index shall contain the complete normalized project model
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-NAVIGATE-TRACE
:derives_from: GOAL-AGENT-READY

The index shall contain project and schema identity, documents, sections,
narrative blocks, item placements, raw bodies, typed fields, canonical MID
edges, derived source-span nodes and edges, inverse presentation, weak mentions,
external nodes, source spans, relation provenance, and available Git provenance.
:::

:::req m_01KY7YA2CYFMXA5JXG2PV7ADB8
:id: REQ-INDEX-DETERMINISTIC
:title: The JSON index shall be byte-deterministic for equivalent input
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: GOAL-AUDITABLE

Mara shall define stable ordering and serialization for every index collection
and object. It shall omit timestamps, absolute machine paths, random values, and
other host-specific data that would change bytes for equivalent project input
and equivalent relevant Git state.
:::

:::req m_01KY7YA2CZG8TYWC1M77KDA3N2
:id: REQ-INDEX-GIT-PROVENANCE
:title: The index shall record available relevant Git provenance
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: GOAL-AUDITABLE
:mitigates: RISK-INDEX-AUTHORITY

Inside a worktree, the index shall record HEAD commit, branch when available,
the project path relative to repository root, and whether relevant project files
are modified or untracked. It shall not embed the host's absolute repository
path. Outside Git these fields shall be explicitly unavailable.
:::

:::req m_01KY7YA2D044X5TE8Z9JYNC5NQ
:id: REQ-INDEX-FULL-REBUILD
:title: v0.1 shall rebuild the complete model on every command
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-AUDITABLE

v0.1 query, validation, and index commands shall parse and normalize the full
selected project without persistent caches, file hashes, Git-diff shortcuts, or
incremental graph mutation. Caching shall require later profiling and shall not
change semantic results.
:::

## Portability and scale

:::req m_01KY7YA2D15KMVGMMMQ9NZ5B5S
:id: REQ-PORTABLE-CLI
:title: Mara v0.1 shall support Linux, macOS, and Windows
:status: approved
:level: system
:kind: quality
:priority: should
:derives_from: GOAL-SCALABLE

The CLI and reusable libraries shall support current stable Rust on Linux,
macOS, and Windows with equivalent path normalization, document semantics,
diagnostic codes, and normalized JSON output.
:::

:::decision m_01KYDMFST0EY73VM8ZVCNZKHV5
:id: ADR-0014
:title: Defer Windows runtime qualification from bootstrap delivery
:status: accepted
:kind: process
:justifies: REQ-PORTABLE-CLI

Mara v0.1 bootstrap delivery shall proceed on non-Windows hosts while Windows
runtime support remains unclaimed and unverified. Windows is not a supported
Mara v0.1 runtime until the dedicated portability qualification has completed.
Incidental compilation, cross-compilation, or partial compatibility is not
verification evidence and does not constitute a support guarantee.

The dedicated Windows delivery shall establish native behavioural CI and close
the remaining platform-specific implementation and verification work before
Mara claims Windows runtime support. Deferring that qualification does not block
the current non-Windows bootstrap scope.
:::

:::req m_01KY7YA2D2VJ961QCVMGTNNK21
:id: REQ-PERFORMANCE-TARGET
:title: Full validation shall meet the v0.1 scale budget
:status: approved
:level: system
:kind: performance
:priority: should
:derives_from: GOAL-SCALABLE

On a documented CI reference runner, a clean full check of 10,000 items and
100,000 resolved edges shall complete within five seconds and use no more than
512 MiB of resident memory.
:::

## Design, rationale, risk, and artifacts

:::design m_01KZ7YA2D7EJ73VM8ZVCNZKHV5
:id: DES-V01-QUALIFICATION-METHOD
:title: Proposed v0.1 scale and non-Windows qualification method
:status: proposed
:kind: cli
:satisfies: REQ-PERFORMANCE-TARGET
:refines: REQ-PORTABLE-CLI
:relates_to: TEST-PORTABILITY-PERFORMANCE

This proposed method makes the v0.1 qualification of
[[TEST-PORTABILITY-PERFORMANCE]] reproducible without changing the approval
state of that test, [[REQ-PERFORMANCE-TARGET]], or [[ADR-0014]]. It becomes
authoritative only after the human approval recorded for CON-46 and acceptance
of this design in the Git-tracked Mara corpus; until both occur it is a
reviewable proposal and no passing evidence or platform-support claim may rely
on it.

### Reference runner and evidence

The v0.1 performance reference runner is a GitHub-hosted
`ubuntu-24.04` `x64` GitHub Actions runner. The qualification must not use the
moving `ubuntu-latest` label or a self-hosted runner as the reference runner.
Its immutable evidence record shall identify the exact source commit containing
the release binary and fixture, and retain the complete command output and
measurement files. It shall also record the workflow run URI and identifier,
the `ubuntu-24.04` label, `RUNNER_OS`, `RUNNER_ARCH`, `RUNNER_NAME`, `ImageOS`,
and `ImageVersion`; the raw output of `uname -a`, `lscpu`, and `free -b`; the
`rustup show active-toolchain`, `rustc -Vv`, `cargo -V`, `python3 --version`,
and `/usr/bin/time --version` output; and the release binary SHA-256. These
values document the actual image, CPU model and logical CPU count, memory
capacity, toolchain, and measurement utility rather than assuming that a
hosted-runner label fixes them permanently.

### Deterministic scale fixture

The checked-in source project at `tests/fixtures/scale-v01` shall contain
exactly ten document files named `items-000.mara.md` through
`items-009.mara.md`, its ordinary Mara project configuration and schema, and no
generated index or cache. Its committed `SHA256SUMS` manifest shall list
`.mara/project.toml`, `.mara/schema.yaml`, and every fixture source file except
itself, pinning the exact fixture bytes and therefore its required ID set,
relation declarations, validation rules, and relation topology. The documents
shall contain exactly 10,000 distinct items with
display IDs `SCALE-00000` through `SCALE-09999`; each item has a fixed, distinct
MID in the committed source. For each integer `i` from 0 through 9,999, the
five-digit `SCALE` display ID for `i` shall have exactly ten `depends_on`
relations to the five-digit `SCALE` display IDs for `(i + d) modulo 10,000`,
for each `d` from 1 through 10. The formula has no self relation or repeated
source-target pair, so a successful full check resolves exactly
10,000 × 10 = 100,000 directed edges.

Before every qualification run, the following ordinary shell checks shall prove
the authored fixture counts; the `mara check` summary then independently proves
that those authored references resolve to 10,000 items and 100,000 edges in the
fixture project:

```sh
set -e
repo_root=$(git rev-parse --show-toplevel)
fixture="$repo_root/tests/fixtures/scale-v01"
case "$(uname -s)" in
  Linux) (cd "$fixture" && sha256sum --check SHA256SUMS) ;;
  Darwin) (cd "$fixture" && shasum -a 256 -c SHA256SUMS) ;;
  *) exit 1 ;;
esac
test "$(grep -hE '^:id: SCALE-[0-9]{5}$' "$fixture"/items-*.mara.md | wc -l | tr -d '[:space:]')" -eq 10000
test "$(grep -hE '^:depends_on: SCALE-[0-9]{5}$' "$fixture"/items-*.mara.md | wc -l | tr -d '[:space:]')" -eq 100000
awk '
  function fail(message) { print "scale-v01 topology: " message > "/dev/stderr"; bad = 1 }
  function clear_edges(  target) { for (target in edge) delete edge[target] }
  function finish_item(  source_index, d, target) {
    if (source == "") return
    if (seen[source]++) fail("duplicate source " source)
    if (edge_count != 10) fail(source " has " edge_count " depends_on relations")
    source_index = substr(source, 7) + 0
    for (d = 1; d <= 10; d++) {
      target = sprintf("SCALE-%05d", (source_index + d) % 10000)
      expected[target] = 1
      if (!(target in edge)) fail(source " is missing " target)
    }
    for (target in edge) if (!(target in expected)) fail(source " has unexpected " target)
    for (target in expected) delete expected[target]
    item_count++
  }
  /^:id: / {
    finish_item()
    source = $2
    if (source !~ /^SCALE-[0-9][0-9][0-9][0-9][0-9]$/) fail("invalid source " source)
    edge_count = 0
    clear_edges()
    next
  }
  /^:depends_on: / {
    if (source == "") fail("relation before source")
    edge[$2]++
    edge_count++
  }
  END {
    finish_item()
    if (item_count != 10000) fail("expected 10000 items, found " item_count)
    exit bad
  }
' "$fixture"/items-*.mara.md
check_output=$(mktemp)
(cd "$fixture" && "$repo_root/target/release/mara" check --format json) > "$check_output"
grep -Eq '^[[:space:]]*"status": "ok",' "$check_output"
grep -Eq '^[[:space:]]*"diagnostics": \[\],' "$check_output"
grep -Eq '^[[:space:]]*"documents": 10,' "$check_output"
grep -Eq '^[[:space:]]*"items": 10000,' "$check_output"
grep -Eq '^[[:space:]]*"edges": 100000,' "$check_output"
cat "$check_output"
rm "$check_output"
```

This proposal introduces no persisted fixture generator, benchmark helper, or
verification tool. It does propose the non-persisted inline `awk` topology
assertion above as a narrowly bounded custom verification helper. Its invariant
is that the committed fixture must have exactly the prescribed ten outgoing
`depends_on` edges for every `SCALE` item; the manifest proves fixed source
bytes and the count checks prove only aggregate counts, so neither alone
independently checks that topology. The helper's only input is the ten
`items-*.mara.md` files; it reads only `:id:` and `:depends_on:` lines, writes a
diagnostic to standard error on a violated invariant, and exits non-zero on any
violation. It has no generated output, stored state, command-line API, or
runtime dependency beyond POSIX `awk`; its maintenance scope is this method and
the fixed fixture contract, and it may change only when the fixture invariant
changes through a separately approved contract.

The Mara product/architecture owner must explicitly approve this helper with
the rest of this proposed method before any delivery issue implements or relies
on it. If a future delivery requires any other custom generator, helper, or
verification tool, it must stop before implementation and record its invariant,
why the fixed fixture and ordinary mechanisms are insufficient, the proposed
tool, its bounded syntax or API surface, its bounded maintenance scope, and
explicit human approval alongside an accepted applicable Mara contract, as
required by [[REQ-VERIFICATION-LAYERS]].

### Native behavioural and performance procedure

The native behavioural matrix consists of GitHub-hosted `ubuntu-24.04` and
`macos-14` jobs. Each job shall first assert its native host with
`test "$(uname -s)" = Linux` or `test "$(uname -s)" = Darwin`, respectively;
select current stable Rust with
`rustup toolchain install stable --profile minimal --component rustfmt --component clippy`
and `rustup default stable`; run `cargo fmt --all -- --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, and
`cargo test --all-targets --all-features`; build the default native target with
`cargo build --locked --release --bin mara`; and run the fixture-count
checks and clean `mara check` invocation above. A successful fixture check has
exit status 0, JSON `status` `ok`, and no diagnostics. The macOS job records its
workflow URI, commit, `sw_vers`, `uname -a`, `sysctl -n hw.logicalcpu`,
`sysctl -n hw.memsize`, and Rust toolchain version with the behavioural result.
Neither cross-compilation nor a job whose asserted host differs from its matrix
entry is native behavioural evidence.

On the Linux reference runner only, perform five measured clean checks after
the release build and fixture-count checks. A clean check means a new `mara`
process over the checked-in fixture with no Mara-generated index or cache; it
does not require privileged operating-system cache flushing. Run this exact
procedure from a clean source worktree:

```sh
set -euo pipefail
repo_root=$(git rev-parse --show-toplevel)
fixture="$repo_root/tests/fixtures/scale-v01"
binary="$repo_root/target/release/mara"
results_dir=$(mktemp -d)
test -z "$(git -C "$repo_root" status --porcelain=v1)"

for run in 1 2 3 4 5; do
  start_ns=$(python3 -c 'import time; print(time.monotonic_ns())')
  (
    cd "$fixture"
    /usr/bin/time --output="$results_dir/run-$run.time" \
      --format='peak_rss_kib=%M' \
      "$binary" check --format json > "$results_dir/run-$run.json"
  )
  end_ns=$(python3 -c 'import time; print(time.monotonic_ns())')
  { printf 'elapsed_nanoseconds=%s\n' "$((end_ns - start_ns))"; cat "$results_dir/run-$run.time"; } \
    > "$results_dir/run-$run.measurement"
  grep -Eq '^[[:space:]]*"status": "ok",' "$results_dir/run-$run.json"
  grep -Eq '^[[:space:]]*"diagnostics": \[\],' "$results_dir/run-$run.json"
done

awk -F= '$1 == "elapsed_nanoseconds" { n++; if ($2 + 0 > 5000000000) bad = 1 } END { exit (n != 5 || bad) }' \
  "$results_dir"/run-*.measurement
awk -F= '$1 == "peak_rss_kib" { n++; if ($2 + 0 > 524288) bad = 1 } END { exit (n != 5 || bad) }' \
  "$results_dir"/run-*.measurement
```

Python's `time.monotonic_ns()` records elapsed time against a monotonic clock as
an integer number of nanoseconds, while `/usr/bin/time` reports peak resident
set size in KiB as `%M`; 5 seconds is 5,000,000,000 nanoseconds and 512 MiB is
524,288 KiB. The performance pass calculation is strict: all five `mara check`
processes must exit successfully with JSON `status` `ok` and no diagnostics,
all five measurement files must contain one value for each metric, the greatest
elapsed time must be at most 5,000,000,000 nanoseconds, and the greatest peak
RSS must be at most 524,288 KiB. The evidence record shall retain all five JSON
and measurement files, not only a selected or median result.

Windows has no job or support claim in this proposal. Native Windows behavioural
qualification remains deferred by [[ADR-0014]]; passing the Linux and macOS
matrix or any Windows cross-compilation does not close that boundary. After an
approved method is executed, the resulting immutable evidence item must link to
[[TEST-PORTABILITY-PERFORMANCE]] and the exact verified commit in accordance
with [[REQ-EVIDENCE-REVISION-ANCHOR]]. Its observable reassessment signal for
CON-32 is a five-run Linux result satisfying both maxima together with passing
native Linux and macOS behavioural records. Linear retains the CON-32 blocker
and scheduling state and uses that signal for reassessment.
:::

:::design m_01KY7YA2D74S6NC0FGAZ2ZKFT2
:id: DES-QUERY-INDEX
:title: Shared in-memory project model with projection adapters
:status: accepted
:kind: architecture
:satisfies: REQ-QUERY-RESOLUTION
:satisfies: REQ-QUERY-FORMATS
:satisfies: REQ-LIST-ITEMS
:satisfies: REQ-LIST-FILTERS
:satisfies: REQ-SHOW-ITEM
:satisfies: REQ-TRACE-DIRECTION
:satisfies: REQ-TRACE-DEPTH
:satisfies: REQ-INDEX-COMMAND
:satisfies: REQ-INDEX-CONTENT
:satisfies: REQ-INDEX-DETERMINISTIC
:satisfies: REQ-INDEX-GIT-PROVENANCE
:satisfies: REQ-INDEX-FULL-REBUILD
:satisfies: REQ-PORTABLE-CLI
:satisfies: REQ-PERFORMANCE-TARGET

All read commands shall consume one immutable normalized project model. Human
formatters, JSON command output, and the index writer are presentation adapters
over that model, preventing CLI-specific semantics from diverging from future
library and web consumers.
:::

:::decision m_01KY7YA2D87T12812J64ZMN1GF
:id: ADR-0007
:title: Rebuild before introducing index complexity
:status: accepted
:kind: architecture
:justifies: DES-QUERY-INDEX
:justifies: REQ-INDEX-FULL-REBUILD

A complete deterministic rebuild is simple to reason about, verifies that Git
source is sufficient, and provides a correctness reference for future caching,
SQLite, or graph projections. Performance optimization follows measured need.
:::

:::risk m_01KY7YA2D9S1Y8RXZTQYPQMGFN
:id: RISK-INDEX-AUTHORITY
:title: Consumers may mistake a generated index for authoritative data
:status: open
:severity: high
:likelihood: medium
:affects: REQ-INDEX-COMMAND
:affects: REQ-INDEX-CONTENT

Fast consumers may be tempted to mutate or preserve an index independently of
source, allowing stale or modified derived state to appear authoritative.
:::

:::artifact m_01KY7YA2DANZ8F7690GGWEM9TS
:id: ART-MARA-CLI
:title: Mara command-line interface
:status: proposed
:kind: command
:uri: crates/mara-cli

The CLI exposes initialization, MID generation, validation, inspection,
traversal, indexing, and display-ID editing over reusable Mara libraries.
:::

:::artifact m_01KY7YA2DBYSDJ0MD0CTJAP23Z
:id: ART-JSON-PROJECTION
:title: Versioned JSON projection format
:status: proposed
:kind: file_format
:uri: .mara/index.json

The deterministic JSON format is the first complete machine-facing projection
of a normalized Mara project.
:::

## Planned verification

:::test m_01KY7YA2D31YRA5J3FVWDZPDCM
:id: TEST-LIST-SHOW
:title: Item listing and inspection acceptance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-QUERY-RESOLUTION
:verifies: REQ-QUERY-FORMATS
:verifies: REQ-LIST-ITEMS
:verifies: REQ-LIST-FILTERS
:verifies: REQ-SHOW-ITEM

CLI fixtures shall cover MID and display-ID resolution, deterministic listing,
OR within repeated flavour and field values, AND across field names, normalized
effective filter JSON including empty and cross-flavour mixed-type filters,
unknown and unconvertible filter errors, repeatable-field existential matching,
absent fields, duplicate values, numeric negative-zero equality, self-contained
show mentions, human snapshots, and stable JSON data. Invalid filters and
unresolved or ambiguous `show` references shall produce `status: invalid` with
null `data` and null `error` while retaining exact diagnostics.
:::

:::test m_01KY7YA2D4ZH3EBGK1FN9BBBYC
:id: TEST-TRACE
:title: Bounded trace traversal acceptance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-TRACE-DIRECTION
:verifies: REQ-TRACE-DEPTH

Graph fixtures containing branches, cycles, inverse authoring, and repeated
paths shall verify direction, depth, path reporting, cycle safety, and stable
ordering. Every JSON path step shall expose canonical source and target plus the
actual incoming or outgoing traversal direction. Exact golden path sets shall
cover a directed cycle, a diamond, and two different canonical relations between
the same endpoints; they shall prove simple-path exclusion of repeated node
identities, absence of the zero-step focus path, occurrence deduplication, and
relation-based parallel-path preservation. Additional fixtures shall traverse
from an item to an external target and from an item backlink to a derived
source-span node, proving that non-item endpoint identities appear in path steps
and terminate safely. The derived node shall be injected at the normalized-model
fixture seam; this test does not require a configured source scanner.
Unresolved or ambiguous focus references shall produce `status: invalid` with
null `data` and null `error` while retaining exact diagnostics.
:::

:::test m_01KY7YA2D55K5KDK24KXTBWJVF
:id: TEST-JSON-INDEX
:title: Deterministic JSON projection acceptance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-INDEX-COMMAND
:verifies: REQ-INDEX-CONTENT
:verifies: REQ-INDEX-DETERMINISTIC
:verifies: REQ-INDEX-GIT-PROVENANCE
:verifies: REQ-INDEX-FULL-REBUILD

Golden indexes shall cover complete document and graph content, clean and dirty
Git states, unversioned directories, repeated rebuilds, changed filesystem
enumeration order, exact v1 key and collection ordering, null policy, canonical
UTF-8 serialization, and absence of machine-specific absolute paths. Policy
fixtures shall prove that non-failing warnings are serialized, warning escalation
suppresses generation, every failing diagnostic leaves any existing index
unchanged, and a later successful rebuild atomically replaces it. Independent
normalized-model fixtures shall prove exact `source_nodes` projection, `NodeRef`
endpoint encoding, incoming item backlinks, deterministic deduplication, and
absence of invented MIDs for source spans. Scanner discovery and language-aware
marker recognition remain in the future-adapter test. Fault-injected writer
fixtures shall cover temporary serialization, file flush, atomic replacement,
and parent-directory flush, proving that the configured path contains the
complete previous or complete new index and never a partial document.
When validation policy suppresses an index write, the JSON command envelope
shall use `status: invalid`, null `data`, and null `error`, and the fixture shall
prove that no new index or hash is reported and any previous index is unchanged.
:::

:::test m_01KY7YA2D6CG9DXA8CBGGV3B5G
:id: TEST-PORTABILITY-PERFORMANCE
:title: Cross-platform and scale qualification test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-PORTABLE-CLI
:verifies: REQ-PERFORMANCE-TARGET

The dedicated portability qualification shall run behavioural suites on Linux,
macOS, and Windows before support is claimed for those runtimes. It shall also
benchmark the documented 10,000-item, 100,000-edge fixture on the designated
reference runner against time and memory budgets. Passing non-Windows CI or
cross-compilation alone shall not verify Windows runtime support.
:::
