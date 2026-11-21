---
title: Link this repository's code to the requirements it implements
status: in-progress
type: task
parent: mara-779tms
blocked_by:
  - mara-9fphtg
discovered_from: mara-779tms
created_at: 2026-11-21T19:15:35Z
---

Goal: the outcome of the 0.4.0-alpha.5 epic holds for this repository itself: its configuration declares Rust as a code language, its code carries `@mara` markers on what implements each requirement, and Mara's own reads show those links.

Context: the epic mara-779tms is "Done when this repository's code links to the requirements it implements". At `d6da3c9` no issue did that: `.mara/config.toml` has no `[code]` table, and the only `@mara` text under `crates/` is in tests, fixtures and messages. Filed by the orchestrator on 2026-11-21. Items: REQ-CODE-MARKER-LINKS, DES-CODE-MARKERS and DES-CODE-READERS (docs/code/index.mara.md); the query `implemented` and the gate `verified` of the engineering template (`.mara/config.toml`, `.mara/rules/readiness.dl`); the relation the vocabulary gives a code link (docs/template/vocabulary.mara.md).

Decided by the orchestrator, provisional (the maintainer may overturn it): the repository declares Rust at the basic level, without a pinned grammar. With a grammar pinned, `project validate`, one of the local checks, fails with `unavailable-grammar` on every machine that has not run `mara code add rust`, which downloads; the local checks stay free of downloads until the maintainer says otherwise. Moving to the declaration level later is `mara code add rust` and no change to the markers.

- [ ] `.mara/config.toml` declares `[code]` with Rust at the basic level; its include patterns cover the crates' source and leave out files whose `@mara` text is test data (fixtures, test strings), so that no marker resolves to nothing
- [ ] Every Accepted requirement that the code implements today has at least one `@mara implements <ID>` marker, at the function, type or module that implements it; no marker claims a requirement that is not built yet. Requirements only: markers for designs and verifications are a follow-up issue if they prove useful
- [ ] `project validate` reports no error and no new warning; `query execute implemented` lists the links; the closing summary lists each Accepted requirement that has no marker, with the slice or issue that will build it
- [ ] AGENTS.md says in one line, under "Mara documents", that code implementing a requirement carries its marker and that markers change with the code; README and other pages only link

Done when `cargo run --package mara --locked --quiet -- --no-daemon query execute implemented` in this repository lists a code link for each requirement the summary names as implemented, and the seven local checks pass.
