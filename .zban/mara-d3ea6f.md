---
title: Regroup docs into subject folders and drop the page size limit
status: in-progress
type: task
priority: high
blocked_by:
  - mara-yhxq5x
  - mara-17gykj
  - mara-274e1m
  - mara-gfb6n6
created_at: 2026-11-16T17:58:13Z
---

Decided by the maintainer on 2026-11-16 (relayed through the zban coordinator thread).

The maintainer dropped the 200-line page limit and wants `docs/` laid out by the `mara-docs-layout` skill (a personal skill installed globally at `~/.agents/skills/mara-docs-layout/SKILL.md`; it is not the old `mara` skill that AGENTS.md forbids). The 200-line limit forced repeated splits into 39 flat pages with name-prefix families (`rules-*`, `verification-*`, `decisions-*`, `template-*`, `retrieval-*`, `authoring-*`). The skill's stage 3 calls for subject folders instead: a subject that needs more than one page, counting its decisions and verification, gets `docs/<subject>/` with an `index.mara.md` overview and pages named without the prefix. Decisions and checks that span subjects stay in the root `decisions.mara.md` and `verification.mara.md`.

- [ ] In `AGENTS.md`, remove "any docs page ≤ 200 lines (split it otherwise)" and the line saying the 200-line limit replaces the skill's 500-line signal, and change `docs/*.mara.md` to `docs/**/*.mara.md`
- [ ] Group pages into subject folders per the skill. Starting point, adjust where the content says otherwise: `rules/` (rules, rule facts, rule evaluation, rules and program-safety verification, rules decisions), `authoring/` (authoring, write requests, write safety and their verification), `retrieval/` (retrieval, read requests, retrieval verification), `code/` (code links and both code-link verification pages), `template/` (knowledge rules, readiness, vocabulary), `daemon/` and `hosts/` (each page plus its verification). Single-page subjects stay in the root.
- [ ] Merge pages that were split only to meet the 200-line limit, where the result stays under about 500 lines
- [ ] Move items with the worktree binary's move operation so IDs and MIDs stay unchanged; change no item text
- [ ] Update every relative link and anchor (docs, `AGENTS.md`, `README.md`) and every folder `index.mara.md`; `docs/index.mara.md` lists root pages and one line per subject folder
- [ ] Project validation with the worktree binary reports valid with complete evaluation, and `zban check` passes

Done when the checks above pass and the change contains only moves, merges, link and index updates and the `AGENTS.md` edits.

Order, decided by the maintainer on 2026-11-16: the four issues in progress (mara-yhxq5x, mara-17gykj, mara-274e1m, mara-gfb6n6) are integrated first and no other issue starts meanwhile; then this issue runs alone on the quiet tip, so that no branch has to rebase over the moves; then the other work resumes.
