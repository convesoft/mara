---
title: Regroup docs into subject folders and drop the page size limit
status: done
type: task
priority: high
blocked_by:
  - mara-yhxq5x
  - mara-17gykj
  - mara-274e1m
  - mara-gfb6n6
created_at: 2026-11-16T17:58:13Z
closed_at: 2026-11-16T20:13:19Z
---

Decided by the maintainer on 2026-11-16 (relayed through the zban coordinator thread).

The maintainer dropped the 200-line page limit and wants `docs/` laid out by the `mara-docs-layout` skill (a personal skill installed globally at `~/.agents/skills/mara-docs-layout/SKILL.md`; it is not the old `mara` skill that AGENTS.md forbids). The 200-line limit forced repeated splits into 39 flat pages with name-prefix families (`rules-*`, `verification-*`, `decisions-*`, `template-*`, `retrieval-*`, `authoring-*`). The skill's stage 3 calls for subject folders instead: a subject that needs more than one page, counting its decisions and verification, gets `docs/<subject>/` with an `index.mara.md` overview and pages named without the prefix. Decisions and checks that span subjects stay in the root `decisions.mara.md` and `verification.mara.md`.

- [x] In `AGENTS.md`, remove "any docs page ≤ 200 lines (split it otherwise)" and the line saying the 200-line limit replaces the skill's 500-line signal, and change `docs/*.mara.md` to `docs/**/*.mara.md`
- [x] Group pages into subject folders per the skill. Starting point, adjust where the content says otherwise: `rules/` (rules, rule facts, rule evaluation, rules and program-safety verification, rules decisions), `authoring/` (authoring, write requests, write safety and their verification), `retrieval/` (retrieval, read requests, retrieval verification), `code/` (code links and both code-link verification pages), `template/` (knowledge rules, readiness, vocabulary), `daemon/` and `hosts/` (each page plus its verification). Single-page subjects stay in the root.
- [x] Merge pages that were split only to meet the 200-line limit, where the result stays under about 500 lines
- [x] Move items with the worktree binary's move operation so IDs and MIDs stay unchanged; change no item text
- [x] Update every relative link and anchor (docs, `AGENTS.md`, `README.md`) and every folder `index.mara.md`; `docs/index.mara.md` lists root pages and one line per subject folder
- [x] Project validation with the worktree binary reports valid with complete evaluation, and `zban check` passes

Done when the checks above pass and the change contains only moves, merges, link and index updates and the `AGENTS.md` edits.

Order, decided by the maintainer on 2026-11-16: the four issues in progress (mara-yhxq5x, mara-17gykj, mara-274e1m, mara-gfb6n6) are integrated first and no other issue starts meanwhile; then this issue runs alone on the quiet tip, so that no branch has to rebase over the moves; then the other work resumes.

## Summary of Changes

- `docs/` follows stage 3 of the `mara-docs-layout` skill: 14 root pages and the subject folders `authoring/`, `retrieval/`, `rules/`, `template/`, `hosts/`, `daemon/`, `code/`, each with an `index.mara.md` that opens with its page list. `docs/index.mara.md` lists the root pages and one line per folder.
- Whole pages moved with `git mv`, then `mara project fix` (worktree binary) rewrote 355 stale link paths; items of merged or dissolved pages moved one by one with `mara item move`. No item text and no narrative text changed: the only added or dropped lines are page titles of dissolved pages, the empty section headings they left, the section headings `## Decisions`, `## Verification`, `## Evidence`, `## Risks` that receive moved items, and the index tables.
- `AGENTS.md`: the 200-line page limit and the clause that it replaces the skill's 500-line signal are gone; the glob reads `docs/**/*.mara.md`; the vocabulary link points to its new page. `README.md`: links to the init and retrieval pages follow them. `.mara/config.toml` already includes `docs/**/*.mara.md` and is unchanged.
- Doc comments in `crates/` and `scripts/` that cite a page by path were substituted by the table below; two whose items split across pages were adjusted by hand (`crates/mara/src/daemon.rs`, `crates/mara/tests/mcp/main.rs`). The `include_str!` paths of four tests that read this repository's pages follow their pages. `crates/mara/tests/documents.rs` listed only the top level of `docs/` and now walks its subfolders, so it still checks every page.

### Old page → new page

| Old page | New page |
| --- | --- |
| `agents`, `decisions`, `distribution`, `engine`, `format`, `identity`, `index`, `migration`, `performance`, `product`, `project`, `validation`, `verification` | unchanged |
| `init` | `project` |
| `authoring`, `authoring-requests` | `authoring/index` |
| `writes` | `authoring/writes` |
| `verification-authoring`, `verification-writes` | `authoring/verification` |
| `retrieval`, `retrieval-requests` | `retrieval/index` |
| `verification-retrieval` | `retrieval/verification` |
| `rules`, `rules-facts` | `rules/index` |
| `rules-evaluation`, `rules-evidence` | `rules/evaluation` |
| `decisions-rules` | `rules/decisions`, except ADR-VALIDITY-NOT-READINESS, ADR-NASA-INFORMED-RISK-MATRIX, ADR-HISTORICAL-SUPERSESSION (`template/index`) and ADR-RESULTS-BOUND-TO-BOTH-REVISIONS (`template/readiness`) |
| `verification-rules`, `verification-programs` | `rules/verification` |
| `template` | `template/index` |
| `template-readiness` | `template/readiness` |
| `vocabulary` | `template/vocabulary` |
| `code` | `code/index` |
| `verification-code`, `verification-code-use` | `code/verification` |
| `daemon` | `daemon/index` |
| `verification-daemon` | `daemon/verification` |
| `hosts` | `hosts/index` |
| `verification-browser` | `hosts/verification`, except VER-UNAVAILABLE-NOT-PARTIAL (`verification`) |
| `decisions-runtime` | ADR-CONTENT-KEYED-VALUES, ADR-GITIGNORE-FILTERS-INPUTS, ADR-COUNTERS-GATE-PULL-REQUESTS, ADR-SCALING-CPU-TIME, ADR-HUB-ROW-KEYS, ADR-NO-CONVERTER-SCALING → `decisions`; ADR-MEMORY-ONLY-CACHES, ADR-ON-DEMAND-DAEMON, ADR-DAEMON-PER-VERSION, ADR-HTTP-BY-FLAG → `daemon/index`; ADR-COMMENT-MARKERS-NOT-SCIP → `code/index` |
| `verification-documents` | `verification` |
| `verification-hosts` | `verification`, except VER-CLI-MCP-PARITY (`hosts/verification`) |
| (`decisions`) | ADR-NO-MUTATION-JOURNAL → `authoring/writes`, ADR-GROUP-VERB-COMMANDS → `hosts/index` |
| (`verification`) | VER-TEMPLATE-CASES → `template/index` |

All paths are `docs/<page>.mara.md`. Items are found by ID in any case (`mara get <ID>`).

## Decisions made

- Merged the pages split only for the 200-line limit, each result under 310 lines: `rules-facts` into `rules/index` (commit 61874e7 says the rules pages were split for the limit), `rules-evidence` into `rules/evaluation` under `## Evidence` (0513a47 moved it out of `rules-evaluation`), `authoring-requests` and `retrieval-requests` into their subject's index (new pages next to pages at 183 and ~150 lines), `init` into `project` (project was at 194 lines when init was added), and the subject verification pairs `verification-programs`, `verification-writes`, `verification-code-use` into their subject's `verification` page. `rules/evaluation`, `template/readiness` and `authoring/writes` stay pages: each is a subject other pages link to on its own.
- Differences from the issue's starting grouping, by the skill's rule that a folder holds all of its subject's items and per-subject decisions or verification pages never sit in the root: the four template decisions moved from the rules decisions to `template/`; `decisions-runtime` was dissolved by subject; `verification-documents` and `verification-hosts` were dissolved into the root `verification` page, because their subjects (format, identity, project, validation, init, migration, agents) are single root pages, and VER-CLI-MCP-PARITY went to `hosts/verification`.
- Root `decisions` and `verification` keep the items of single-page root subjects and cross-subject ones (267 and 406 lines, under the skill's 500-line signal); a single-page subject gets no `## Decisions` or `## Verification` section of its own in this change.
- Within a folder, a few decisions or checks go into a `## Decisions` or `## Verification` section on the page they justify or check (the existing pattern of `authoring`, `writes`, `hosts`, `daemon`, `code`); a page only where the material already was one.
- VER-UNAVAILABLE-NOT-PARTIAL follows the item it verifies, REQ-UNAVAILABLE-NOT-PARTIAL on `engine`, to the root `verification`; `hosts/verification` is therefore titled "Host verification".
- The 14 root pages are not grouped further (the skill's root signal is about 15).
- `AGENTS.md` keeps "Layout follows the `mara-docs-layout` skill." and drops only the clause about the 200-line limit.
- README's `project init` template link points at DES-PROJECT-INIT's anchor on `project`, where the init page's only item now is.

## Evidence that only the layout changed

`snapshot.py` (below) lists every item with its ID, MID, type, status and the SHA-256 of its title, rows and content after reducing each link destination to its `#fragment`, plus the item's path. With the path column cut, the lists at the base 65b664a and at the head are equal (227 items):

```sh
B=$(mktemp -d "$TMPDIR/base.XXXX"); git archive 65b664a docs .mara | tar -x -C $B
MARA_NO_DAEMON=1 python3 -I snapshot.py $PWD/target/debug/mara $B   | cut -f2- | sort > $B.tsv
MARA_NO_DAEMON=1 python3 -I snapshot.py $PWD/target/debug/mara $PWD | cut -f2- | sort > $B.head.tsv
diff $B.tsv $B.head.tsv && echo equal
```

```python
#!/usr/bin/env python3
"""Snapshot every item: path, ID, MID, type, status, sha256 of normalised text.
Usage: MARA_NO_DAEMON=1 python3 -I snapshot.py <mara-binary> <project-dir> > out.tsv
Normalisation: in each link [text](dest), dest is reduced to its #fragment
(the page path is dropped), so only link destinations may differ."""
import hashlib, json, re, subprocess, sys
mara, proj = sys.argv[1], sys.argv[2]
def run(*a):
    r = subprocess.run([mara, "--project", proj, "--format", "json", *a], capture_output=True, text=True)
    return json.loads(r.stdout)
items, cur = [], None
while True:
    args = ["item", "list", "--limit", "100"] + (["--cursor", cur] if cur else [])
    d = run(*args); items += d["items"]; cur = d.get("next_cursor")
    if not cur: break
link = re.compile(r"\]\(([^)#\s]*)(#[^)\s]*)?\)")
norm = lambda s: link.sub(lambda m: "](" + (m.group(2) or "") + ")", s)
for it in sorted(items, key=lambda i: i["mid"]):
    g = run("get", it["mid"]); n = g["node"]
    assert g.get("next_cursor") is None, it
    rows = [(r["key"], norm(r["value"])) for r in g["rows"]]
    status = dict(rows).get("Status", "")
    text = json.dumps({"title": n["title"], "rows": rows, "content": norm(g["content"])}, ensure_ascii=False)
    print("\t".join([n["path"], n["id"], n["mid"], n["type"], status, hashlib.sha256(text.encode()).hexdigest()]))
```

Narrative was compared the same way: all non-empty lines of all pages except `docs/index.mara.md`, link destinations reduced to fragments, sorted, before and after; the only differences are the headings and index tables listed above.

## Checks

All from the repository root at the head, each exit 0: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`; `cargo test --workspace --locked`; `cargo run --package mara --locked --quiet -- project validate` (`valid: true`, 0 errors, 0 warnings, no diagnostics); `cargo test --package mara --test performance --release --locked` (19 passed, 2 ignored); `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`; `zban check`. A relative-link check over `docs/`, `README.md` and `AGENTS.md` finds no broken link besides the placeholder examples that were there before (`<path>#<MID>`, `../needs.mara.md` in a code example).
