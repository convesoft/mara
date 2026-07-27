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

### Bounded qualification tool and command surface

The future implementation of this method shall add one Rust workspace package
and binary named `mara-xtask` at `crates/mara-xtask`. It shall have exactly two
operational qualification commands, invoked from the repository root after
`cargo build --locked -p mara-xtask` with the ordinary on-disk workspace
`target/` directory:

```sh
target/debug/mara-xtask qualification generate-scale-v01 \
  --qualification-root "$qualification_root"
target/debug/mara-xtask qualification measure-scale-v01 \
  --qualification-root "$qualification_root"
```

Each command requires exactly one `--qualification-root` argument whose value
is an absolute path; it accepts no count, seed, run-count, limit, executable,
or other operational argument. Unknown, repeated, relative, and trailing
arguments fail. The generation command requires that root not to exist and
creates exactly one qualification root containing fixed `fixture` and
`evidence` children. The measurement command is Linux-only and requires that
existing root and fixture, release binary `target/release/mara`, evidence root
`$qualification_root/evidence`, five-run count, 5,000,000,000-nanosecond time
limit, and 524,288-KiB peak-RSS limit.

The caller derives a job-unique candidate exactly as
`$RUNNER_TEMP/mara-scale-v01-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT-$RUNNER_OS`.
Use of those values affects only the disposable location, never generated
bytes. Before creation, the tool canonicalizes the current source repository
and the candidate's existing parent, constructs the normalized candidate, and
rejects a symlinked final component. After creation, it canonicalizes the root
and repeats the checks. The root must be outside and not beneath the canonical
source repository, every path from `git worktree list --porcelain`, the top
level of any Git worktree containing the candidate parent, and the physically
resolved `/tmp`. The root and fixture must not contain `.git`; from the fixture,
`git rev-parse --show-toplevel` is expected to fail. This no-Git fixture context
is required, while source commit provenance is collected separately from the
source repository.

Using `statfs` and `statvfs` on the candidate parent before creation and on the
created root, the tool rejects Linux `TMPFS_MAGIC` or `RAMFS_MAGIC`, a macOS
filesystem type named `tmpfs` or `ramfs`, and fewer than 2,147,483,648 available
bytes. Before any root exists, every rejected path or storage precondition emits
one LF-terminated JSON object to standard output for retention in the immutable
workflow log, with exactly the keys `format`, `version`, `source_repo`,
`qualification_parent`, `qualification_root`, `filesystem_type`, `total_bytes`,
`available_bytes`, `minimum_available_bytes`, `on_tmpfs_or_ramfs`,
`inside_source_control`, and `error`. `format` is exactly
`mara.qualification.scale-v01.precondition`, `version` is `1`, byte values and
filesystem type may be null only when unobtainable, booleans report any
determined rejection state, and `error` is a stable non-null machine-readable
string. The command then exits non-zero without creating the candidate root.

For an accepted root, the tool records canonical source, parent, and root paths;
platform filesystem type; total bytes; available bytes; and the threshold in
`evidence/storage-before-generation.json` and, before measured runs,
`evidence/storage-before-measurement.json`. Each storage record has exactly the
keys `source_repo`, `qualification_parent`, `qualification_root`,
`filesystem_type`, `total_bytes`, `available_bytes`,
`minimum_available_bytes`, `on_tmpfs_or_ramfs`, `inside_source_control`, and
`passed`. Paths and filesystem type are strings, byte values are unsigned
integers, and the final three values are booleans. A passing record has
`minimum_available_bytes` equal to `2147483648` and its final three values equal
to `false`, `false`, and `true`. Linux identifies the filesystem by its
lowercase hexadecimal `statfs.f_type`; macOS uses lowercase
`statfs.f_fstypename`. `CARGO_TARGET_DIR` must be unset;
Cargo artifacts remain only in the source worktree's ordinary on-disk
`target/`. Neither command may read or write `/tmp`, another tmpfs or ramfs,
another worktree, or a copied Cargo target. Their only source-repository reads
are the exact `target/debug/mara-xtask` executable, exact
`target/release/mara` executable, current Git metadata needed for
`git rev-parse HEAD`, repository-root discovery, and worktree enumeration and
isolation checks, and the fixed expected manifest at
`tests/qualification/scale-v01.SHA256SUMS`.

### Bounded project-oriented integration-test helper

`ProjectSandbox` is a separate shared Rust helper for project-oriented
integration tests only. It is not a `mara-xtask` command, does not enlarge
`mara-xtask` beyond its exactly two qualification commands above, and has no
production command-line surface. It specifically supports Linear CON-47 and
CON-48, both blocked by CON-46 and both blocking CON-32. Existing ad-hoc
`tempfile` fixtures are insufficient because they do not consistently establish
one unique external on-disk project outside the source checkout, an explicit
child working directory, or isolation from inherited Git discovery for those
project-oriented cases.

Its Rust API is limited to creating one canonically external, unique fixture
project in exactly the required `empty`, `configured`, `clean-Git`, or
`dirty-Git` mode; exposing that project path; configuring a test-owned
subprocess with that path as its explicit current directory; and explicit
cleanup. Before configuring a child, it clears inherited `GIT_*` environment
variables and then uses `GIT_CEILING_DIRECTORIES` at the verified sandbox parent
(or an equivalently verified parent-discovery isolation) so no Git repository
above the sandbox is discovered. The helper creates no repository or
Cargo-target copy. Cleanup is the default and reports every deletion failure
with the canonical retained path and error; a bounded opt-in preserve-on-failure
option may retain only that failed sandbox for diagnosis, never a successful one.

Its maintenance scope is only this project-isolation API and those four modes.
It shall not become a general-purpose test framework, fixture generator,
temporary-directory manager, or command runner, and pure parser, domain, and
in-memory tests shall not use it.

### Deterministic generated scale project

The generated corpus is disposable derived state and shall not be committed to
Git. `generate-scale-v01` creates the complete standalone Mara project beneath
`$qualification_root/fixture` without inheriting or copying the source
repository's corpus, schema, configuration, Git data, ignored files, or project
discovery context. Its static project and schema bytes are embedded in the
approved tool source. Apart from the explicit output location, it does not
consult time, randomness, the network, environment variables, locale, or host
data when deriving generated bytes, and writes only these twelve fixture files
in this order:

1. `.mara/project.toml`;
2. `.mara/schema.yaml`;
3. `items-000.mara.md` through `items-009.mara.md` in ascending name order.

The exact `project.toml` bytes are:

```toml
format_version = 1

[project]
name = "mara-scale-v01"
schema = ".mara/schema.yaml"

[content]
include = ["items-*.mara.md"]
exclude = []
respect_gitignore = false
follow_directory_symlinks = false
allow_internal_file_symlinks = false

[index]
path = ".mara/index.json"

[validation]
warnings_as_errors = true

[git]
require_clean_worktree_for_writes = true
```

The exact `schema.yaml` bytes are:

```yaml
format_version: 1
schema:
  name: mara-scale-v01
  version: 0.1.0
identity:
  mid:
    format: ulid
    prefix: m_
flavours:
  scale:
    label: Scale item
    description: Deterministic v0.1 qualification item.
    guidance:
      use_when:
        - Measuring the v0.1 scale qualification workload.
      avoid_when:
        - Representing non-qualification content.
    id:
      required: true
      pattern: 'SCALE-[0-9]{5}'
    title:
      required: true
    body:
      required: false
relations:
  depends_on:
    source:
      flavours: [scale]
    target:
      flavours: [scale]
    same_flavour: true
    self_reference: false
    cardinality:
      outgoing:
        min: 10
        max: 10
      incoming:
        min: 10
        max: 10
rules: []
```

Every generated file is UTF-8 without a byte-order mark, uses LF line endings,
has no trailing spaces, and ends in exactly one LF. Document `items-NNN.mara.md`
contains the 1,000 item indices `NNN × 1,000` through
`NNN × 1,000 + 999` in ascending order. It has no leading blank line, one empty
line between adjacent item blocks, and no empty line after the final block. For
each integer `i` from 0 through 9,999, the exact item-block template is:

```mara
:::scale {mid(i)}
:id: SCALE-{i as five zero-padded decimal digits}
:title: Scale item {i as five zero-padded decimal digits}
:depends_on: SCALE-{(i + 1) modulo 10,000 as five zero-padded decimal digits}
:depends_on: SCALE-{(i + 2) modulo 10,000 as five zero-padded decimal digits}
...
:depends_on: SCALE-{(i + 10) modulo 10,000 as five zero-padded decimal digits}
:::
```

The ellipsis is specification shorthand: output contains all ten relation lines
in ascending `d` order and no ellipsis. `mid(i)` is `m_` followed by the
zero-padded 26-character canonical uppercase Crockford Base32 ULID encoding of
the unsigned 128-bit integer `i + 1`, using alphabet
`0123456789ABCDEFGHJKMNPQRSTVWXYZ`. Thus every MID is fixed, valid, and distinct
without using the ordinary time-and-randomness MID generator. The topology has
no self relation or repeated source-target pair and contains exactly
10,000 × 10 = 100,000 directed resolved edges.

### Digest pin and independent verification

The future tool delivery shall check in only the small expected manifest
`tests/qualification/scale-v01.SHA256SUMS`, not the generated corpus. It has
exactly twelve LF-terminated lines in the file order above. Each line is a
lowercase 64-digit SHA-256, two ASCII spaces, and the fixture-root-relative
generated path. The generator neither writes nor updates this expected
manifest. A change to the generator, static project bytes, item serialization,
MID mapping, or topology requires an explicitly reviewed manifest change and
applicable contract approval.

After generation and before any measured run, ordinary platform SHA-256 tools,
the bounded independent POSIX `awk` assertion, and the already-built release
`mara` binary shall run from the on-disk worktree as follows. The evidence
directory is created before this procedure; no temporary file is used:

```sh
set -uo pipefail
repo_root=$(git rev-parse --show-toplevel)
qualification_root=${QUALIFICATION_ROOT:?QUALIFICATION_ROOT must be set}
fixture="$qualification_root/fixture"
evidence="$qualification_root/evidence"
expected="$repo_root/tests/qualification/scale-v01.SHA256SUMS"
verification_failed=0

expected_entries=$(printf '%s\n' \
  './.mara' \
  './.mara/project.toml' \
  './.mara/schema.yaml' \
  './items-000.mara.md' \
  './items-001.mara.md' \
  './items-002.mara.md' \
  './items-003.mara.md' \
  './items-004.mara.md' \
  './items-005.mara.md' \
  './items-006.mara.md' \
  './items-007.mara.md' \
  './items-008.mara.md' \
  './items-009.mara.md')
actual_entries=$(cd "$fixture" && find . -mindepth 1 -print \
  2> "$evidence/file-set-check.stderr" | LC_ALL=C sort)
{
  printf 'expected:\n%s\n' "$expected_entries"
  printf 'actual:\n%s\n' "$actual_entries"
} > "$evidence/file-set-check.txt"
test "$actual_entries" = "$expected_entries" || verification_failed=1

expected_manifest_paths=$(printf '%s\n' \
  '.mara/project.toml' \
  '.mara/schema.yaml' \
  'items-000.mara.md' \
  'items-001.mara.md' \
  'items-002.mara.md' \
  'items-003.mara.md' \
  'items-004.mara.md' \
  'items-005.mara.md' \
  'items-006.mara.md' \
  'items-007.mara.md' \
  'items-008.mara.md' \
  'items-009.mara.md')
actual_manifest_paths=$(awk '
  {
    hash = substr($0, 1, 64)
    path = substr($0, 67)
    if (length(hash) != 64 || hash ~ /[^0-9a-f]/ ||
        substr($0, 65, 2) != "  " || path == "" ||
        path ~ /[[:space:]]/ || length($0) != 66 + length(path)) bad = 1
    print path
  }
  END { exit bad }
' "$expected" 2> "$evidence/manifest-path-check.stderr")
manifest_parse_exit=$?
{
  printf 'expected:\n%s\n' "$expected_manifest_paths"
  printf 'actual:\n%s\n' "$actual_manifest_paths"
} > "$evidence/manifest-path-check.txt"
test "$manifest_parse_exit" -eq 0 || verification_failed=1
test "$actual_manifest_paths" = "$expected_manifest_paths" \
  || verification_failed=1

file_type_failed=0
test -d "$fixture" && test ! -L "$fixture" || file_type_failed=1
test -d "$fixture/.mara" && test ! -L "$fixture/.mara" \
  || file_type_failed=1
for relative_path in \
  '.mara/project.toml' \
  '.mara/schema.yaml' \
  'items-000.mara.md' \
  'items-001.mara.md' \
  'items-002.mara.md' \
  'items-003.mara.md' \
  'items-004.mara.md' \
  'items-005.mara.md' \
  'items-006.mara.md' \
  'items-007.mara.md' \
  'items-008.mara.md' \
  'items-009.mara.md'
do
  test -f "$fixture/$relative_path" \
    && test ! -L "$fixture/$relative_path" \
    || file_type_failed=1
done
if test "$file_type_failed" -eq 0; then
  printf 'file_types=ok\n' > "$evidence/file-type-check.txt"
else
  printf 'file_types=failed\n' > "$evidence/file-type-check.txt"
  verification_failed=1
fi

git -C "$fixture" rev-parse --show-toplevel \
  > "$evidence/fixture-git-context.stdout" \
  2> "$evidence/fixture-git-context.stderr"
fixture_git_exit=$?
printf 'expected=no-git\nexit_code=%s\n' "$fixture_git_exit" \
  > "$evidence/fixture-git-context.txt"
test "$fixture_git_exit" -ne 0 || verification_failed=1

case "$(uname -s)" in
  Linux) (cd "$fixture" && sha256sum --check "$expected") 2>&1 \
    | tee "$evidence/fixture-sha256-check.txt" || verification_failed=1 ;;
  Darwin) (cd "$fixture" && shasum -a 256 -c "$expected") 2>&1 \
    | tee "$evidence/fixture-sha256-check.txt" || verification_failed=1 ;;
  *)
    printf 'unsupported host\n' > "$evidence/fixture-sha256-check.txt"
    verification_failed=1
    ;;
esac
item_count=$(grep -hE '^:id: SCALE-[0-9]{5}$' "$fixture"/items-*.mara.md \
  2> "$evidence/count-check.stderr" | wc -l | tr -d '[:space:]')
edge_count=$(grep -hE '^:depends_on: SCALE-[0-9]{5}$' "$fixture"/items-*.mara.md \
  2>> "$evidence/count-check.stderr" | wc -l | tr -d '[:space:]')
printf 'items=%s\nedges=%s\n' "$item_count" "$edge_count" \
  > "$evidence/count-check.txt"
test "$item_count" -eq 10000 || verification_failed=1
test "$edge_count" -eq 100000 || verification_failed=1
if awk '
  function fail(message) { print "scale-v01 topology: " message > "/dev/stderr"; bad = 1 }
  function clear_edges(  target) { for (target in edge) delete edge[target] }
  function finish_item(  source_index, d, target) {
    if (source == "") return
    if (seen[source]++) fail("duplicate source " source)
    if (edge_count != 10) fail(source " has " edge_count " depends_on relations")
    source_index = substr(source, 7) + 0
    for (d = 1; d <= 10; d++) {
      target = sprintf("SCALE-%05d", (source_index + d) % 10000)
      expected_edge[target] = 1
      if (!(target in edge)) fail(source " is missing " target)
    }
    for (target in edge) if (!(target in expected_edge)) fail(source " has unexpected " target)
    for (target in expected_edge) delete expected_edge[target]
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
' "$fixture"/items-*.mara.md > "$evidence/topology-check.txt" 2>&1; then
  printf 'topology=ok\n' >> "$evidence/topology-check.txt"
else
  verification_failed=1
fi
(cd "$fixture" && "$repo_root/target/release/mara" check --format json) \
  > "$evidence/preflight-check.json" 2> "$evidence/preflight-check.stderr"
preflight_exit=$?
printf 'exit_code=%s\n' "$preflight_exit" > "$evidence/preflight-check-exit.txt"
test "$preflight_exit" -eq 0 || verification_failed=1
grep -Eq '^[[:space:]]*"status": "ok",' "$evidence/preflight-check.json" \
  || verification_failed=1
grep -Eq '^[[:space:]]*"diagnostics": \[\],' "$evidence/preflight-check.json" \
  || verification_failed=1
grep -Eq '^[[:space:]]*"documents": 10,' "$evidence/preflight-check.json" \
  || verification_failed=1
grep -Eq '^[[:space:]]*"items": 10000,' "$evidence/preflight-check.json" \
  || verification_failed=1
grep -Eq '^[[:space:]]*"edges": 100000,' "$evidence/preflight-check.json" \
  || verification_failed=1
test "$verification_failed" -eq 0
```

The exact entry-set and file-type assertions reject any missing or additional
entry and require the fixture root and `.mara` to be real directories and all
twelve manifest entries to be regular, non-symlink files. The manifest-path
assertion independently requires the twelve paths exactly once, in the
specified order, with the exact lowercase-hash line format. The manifest then
independently pins every generated file byte,
aggregate counts check the authored volume, the `awk` assertion proves every
source's exact ten-edge formula, and `mara check` proves the authored targets
resolve in a valid project. The generator therefore is not its own oracle.

### Rust measurement semantics

`measure-scale-v01` runs only after generation, digest verification, topology
verification, a successful preflight check, and an already completed
`cargo build --locked --release --bin mara`. Its own build, corpus generation,
digest and topology checks, provenance collection, result serialization, and
other runner setup occur outside every measured interval.

For each run number 1 through 5, sequentially and without a warm-up run, the
Rust tool constructs one child command whose current directory is the generated
fixture and whose executable is the absolute
`$repo_root/target/release/mara` path with exact arguments
`check --format json`. After all per-run setup, it records a
`std::time::Instant` immediately before spawning that child and computes
elapsed nanoseconds from the same `Instant` immediately after the child is
reaped. Only the required external-process spawn, execution, and reap lie
inside that interval. Concurrent pipe reads only prevent child-output deadlock;
JSON validation, hashing, thread joins, and evidence writes occur after the
timer stops.

Before `exec`, the child becomes leader of a fresh process group. The parent
polls that exact PID with Linux `wait4(child_pid, ..., WNOHANG, &rusage)`, never
sleeps past the `Instant` deadline, and uses the same deadline value of exactly
5,000,000,000 nanoseconds. If the child is not reaped by the deadline, the tool
sends `SIGKILL` to the fresh child process group and then calls blocking
`wait4(child_pid, ..., 0, &rusage)` to reap the exact child. The run is marked
timed out and failed even if termination and reap succeed. A failure to create
the process group, kill it, or reap the child is an operational capture failure:
the summary is inconclusive, safely obtainable evidence is written, and the
tool must not leave a child or child process group running.

Selecting the spawned PID avoids cumulative `getrusage(RUSAGE_CHILDREN)` and
prevents another child from contributing. Linux defines `rusage.ru_maxrss` as
peak resident set size in KiB. Standard output and standard error use separate
pipes drained concurrently so output cannot deadlock the child. The tool
retains the complete bytes as `run-01.stdout.json` through
`run-05.stdout.json` and `run-01.stderr` through `run-05.stderr`.

For every run, including an attempted run with an operational failure, the tool
writes `run-NN.measurement.json` with exactly these keys: `run`, `elapsed_ns`,
`peak_rss_kib`, `exit_code`, `term_signal`, `timed_out`, `stdout_sha256`,
`stderr_sha256`, `mara_status`, `diagnostic_count`, `documents`, `items`,
`edges`, `passed`, and `error`. `run` is an unsigned integer from 1 through 5;
`elapsed_ns`, `peak_rss_kib`, and all four parsed counts are unsigned integers
or null when unobtainable; `exit_code` and `term_signal` are integers or null;
`timed_out` and `passed` are booleans; each hash is a lowercase 64-digit
SHA-256 string or null when complete stream capture was impossible;
`mara_status` is a string or null; and `error` is null or a stable
machine-readable string. After a normally captured termination, exactly one of
`exit_code` or `term_signal` is non-null and `error` is null. On an operational
capture failure, both termination fields may be null and `error` is non-null.
All five numbered measurement files and run objects are written; if safe
measurement cannot continue, later objects contain null unavailable fields, a
false `passed`, and a non-null `error` instead of omitting a result.

The tool writes `qualification-summary.json` with exactly these keys: `format`,
`version`, `source_commit`, `xtask_sha256`, `mara_sha256`,
`expected_manifest_sha256`, `fixture_files`, `runs`, `max_elapsed_ns`,
`max_peak_rss_kib`, `elapsed_limit_ns`, `peak_rss_limit_kib`, and `result`.
`format` is exactly `mara.qualification.scale-v01`; `version` is the unsigned
integer `1`; `source_commit` is the exact output of `git rev-parse HEAD`; the
three top-level hashes are lowercase 64-digit SHA-256 strings; and the limits
are the unsigned integers `5000000000` and `524288`. `fixture_files` contains
exactly twelve objects in expected-manifest order, each with exactly `path`,
`expected_sha256`, `observed_sha256`, and `matched`: the path and hashes are
strings and `matched` is a boolean. `runs` contains exactly the five per-run
objects in run-number order. Each maximum is the maximum available unsigned
run value, or null if no value was obtained. `result` is exactly `passed`,
`failed`, or `inconclusive`.

A run passes only when the child exits with code 0, has JSON status `ok`, has no
diagnostics, has the expected 10-document, 10,000-item, 100,000-edge summary,
did not time out, has no operational error,
has elapsed time at most 5,000,000,000 nanoseconds, and has peak RSS at most
524,288 KiB. Overall performance passes only when both storage records pass;
the exact entry-set, file-type, manifest-path, no-Git-context, digest, topology,
count, and preflight checks all pass; all twelve observed fixture hashes equal
their expected hashes; all five records exist and pass; the maximum elapsed time
is at most 5,000,000,000 nanoseconds; and the maximum peak RSS is at most
524,288 KiB. No median, selected run, successful subset, or unchecked fixture
may pass. A child-level failure still records that run and continues the
remaining runs when capture remains safe; an operational capture failure writes
an inconclusive summary. `measure-scale-v01` exits zero only when `result` is
`passed`; it exits non-zero after writing all safely obtainable evidence when
`result` is `failed` or `inconclusive`.

### Native qualification, provenance, and cleanup

The native behavioural matrix consists of GitHub-hosted `ubuntu-24.04` and
`macos-14` jobs. From a clean source worktree, each job sets
`qualification_root` to the exact job-unique `$RUNNER_TEMP` path above,
exports the same value as `QUALIFICATION_ROOT`, requires it not to exist,
records the pre-resource state, asserts `uname -s`
equals `Linux` or `Darwin` for its entry, uses current stable Rust, and keeps
`CARGO_TARGET_DIR` unset. It runs `cargo fmt --all -- --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, builds
`mara-xtask`, and invokes `generate-scale-v01` with the exact root argument.
That invocation creates the external root and proves its isolation, filesystem
type, and free space before generating the fixture. The job then creates only
the temporary `$qualification_root/test-tmp` sibling to the fixture and runs:

```sh
TMPDIR="$qualification_root/test-tmp" \
  cargo test --all-targets --all-features
```

The test-workspace directory is deleted immediately after that command. The
job then builds the default native release `mara` in the source repository's
ordinary `target/`, runs the digest, topology, no-Git-context, and preflight
checks above, always invoking the release binary by its canonical absolute path
with current directory `$qualification_root/fixture`. Normal Mara discovery
therefore selects only the standalone generated project. macOS stops after
that native behavioural result. Only the Linux reference runner invokes
`measure-scale-v01`; cross-compilation is never native behavioural or
performance evidence.

The v0.1 performance reference runner is a GitHub-hosted
`ubuntu-24.04` `x64` GitHub Actions runner, never `ubuntu-latest` or a
self-hosted runner. Before measurement it requires both
`test "${RUNNER_ARCH:-}" = X64` and `test "$(uname -m)" = x86_64`; either
failure prevents performance evidence and makes the job non-zero after evidence
capture. The immutable evidence record retains the workflow URI and identifier;
exact verified source/tool commit; `RUNNER_OS`, `RUNNER_ARCH`,
`RUNNER_NAME`, `ImageOS`, and `ImageVersion`; raw `uname -a`, `lscpu`, and
`free -b`; `rustup show active-toolchain`, `rustc -Vv`, and `cargo -V`; hashes
of the tool, release binary, expected manifest, and generated files; canonical
source and external-root paths; filesystem type, total bytes, available bytes,
and free-space threshold; the expected failed fixture Git-discovery result; raw
digest, topology, and preflight results; every per-run standard-output,
standard-error, and measurement file; and the aggregate summary. Source commit,
tool hash, and binary hash are source-repository provenance; the fixture has no
Git provenance and must not inherit the source commit. The macOS behavioural record
additionally retains `sw_vers`, `uname -a`, `sysctl -n hw.logicalcpu`, and
`sysctl -n hw.memsize`. Runner properties are retained as
`runner-properties.txt` in the evidence root.

Before any Cargo command or generation, and again after qualification but
before evidence upload, each job sets
`tmp_root=$(cd -P /tmp && pwd -P)`, records
`df -h /tmp "$repo_root"` and the directory-used KiB from
`du -sk "$tmp_root"`; the exact gate is
`test "$(du -sk "$tmp_root" | awk '{ print $1 }')" -le 1048576`. Resolving
only the `/tmp` command-line path makes the gate inspect its backing directory
on platforms where `/tmp` is a symlink without following symlinks within that
directory. The `df` output records filesystem capacity but does not drive this
content-usage gate. The pre-check also records filesystem type, capacity, and
available bytes for the canonical `$RUNNER_TEMP` parent in the immutable
workflow log; the Rust storage records provide the authoritative generated-root
checks. Before upload the job records `df -h` for `/tmp`, the source repository,
and qualification root; the physically resolved `/tmp` usage; the qualification
root's used KiB; and the platform filesystem type as
`resource-before-cleanup.txt` in the evidence root, and repeats both the
1-GiB `/tmp` and 2,147,483,648-byte external-free-space gates.

Cargo artifacts remain only in the source repository's ordinary on-disk
`target/`. The generated corpus, evidence, and test workspace remain only in
the isolated external qualification root. None may be placed under `/tmp`, a
tmpfs or ramfs, another worktree, or another source-controlled location; no
repository, Cargo target, corpus, or evidence tree is copied. The required
evidence artifact is uploaded from the external root before cleanup, including
failed or inconclusive results. After the upload attempt completes, whether it
succeeds or fails, a finally-style step deletes the entire canonical
`$qualification_root`, records the post-resource state in the retained workflow
log, and requires `test ! -e "$qualification_root"`. An upload failure remains
an overall job failure and is not masked by successful cleanup; a cleanup
failure likewise fails the job and is not masked by successful upload.

The Rust generator is required because Git shall not persist the large corpus
and ordinary Cargo or fixture mechanisms do not define its exact derived bytes
or enforce a standalone, non-Git project on suitable external storage.
The Rust measurement runner is required because Cargo tests and platform jobs do
not measure one external child with monotonic wall time and exact-child Linux
peak RSS. The independent `awk` helper remains required because a manifest and
aggregate counts do not prove the per-item topology. Their bounded maintenance
scope is only the two command surfaces and required external-root argument,
root/worktree/filesystem isolation checks, fixed scale-v0.1 serialization and
digest contract, Linux `Instant`/`wait4` measurement adapter, evidence schema,
and the inline topology assertion. They shall not become general generators,
temporary-directory managers, benchmark frameworks, command runners, or source
analyzers. `ProjectSandbox` remains separately bounded to the
project-oriented integration-test isolation API and four modes stated above.

The exact human approval required from the Mara product/architecture owner is:

> I approve `DES-V01-QUALIFICATION-METHOD`, including the bounded Rust
> `mara-xtask` generator and Linux measurement runner, the independent POSIX
> `awk` topology assertion, and the separate `ProjectSandbox` shared Rust
> helper for only CON-47 and CON-48 project-oriented integration tests; I
> approve their two-command qualification and bounded ProjectSandbox API
> surfaces, and their stated standalone no-Git fixture, storage-isolation,
> fixed-serialization, Git-discovery isolation, digest, measurement, cleanup,
> preserve-on-failure, and maintenance scope; I approve changing this design
> from `proposed` to `accepted` in the Git-tracked Mara corpus.

Until that approval is recorded for CON-46 and this design is accepted, no
implementation issue becomes ready or may rely on the method. This CON-46
revision records only the proposed contract and does not implement
`mara-xtask`, `ProjectSandbox`, fixture generation, measurement, benchmarks,
the expected manifest, fixture data, CI, or product code. Any broader or
different custom tool returns through the full gate in
[[REQ-VERIFICATION-LAYERS]].

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
