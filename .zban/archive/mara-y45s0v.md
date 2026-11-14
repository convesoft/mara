---
title: Index typo candidates for query words longer than 16 characters
status: cancelled
type: task
priority: low
discovered_from: mara-bmc9gc
created_at: 2026-11-14T19:21:51Z
closed_at: 2026-11-14T19:56:05Z
---

Goal: a typo-tolerant query word longer than 16 characters finds its candidates through an index whose work does not grow with the number of long words.

Context: DES-SNAPSHOT (docs/engine.mara.md) indexes words of up to 18 characters by symmetric deletion (`crates/mara-engine/src/snapshot/spelling.rs`). A word makes about n²/2 strings by two deletions, so longer words are kept by length instead, and a query word longer than 16 characters is checked against every indexed word whose length is within two of its own. A corpus with many distinct long words (hashes, long identifiers) and a query of many long words then does work proportional to their product, against REQ-NEAR-LINEAR-SCALING. Found while fixing mara-bmc9gc, whose scaling cases use 7- and 9-letter words.

- [ ] Choose a candidate filter for long words from a published description, such as a q-gram count filter, that misses no word within two edits (REQ-TYPO-TOLERANT-SEARCH)
- [ ] A scaling case with many distinct long words and a query of many long words gates it (VER-SCALING)
- [ ] DES-SNAPSHOT names the filter

## Reasons for Cancelling

Settled by the maintainer's decision in mara-bmc9gc's third round, which chose "Exact match only above 18 (Recommended) — Typo tolerance applies to query words of up to 18 characters; a longer word must match exactly. The requirement is narrowed by one sentence, the slow path and its code disappear, and the follow-up issue mara-y45s0v is closed as settled." REQ-TYPO-TOLERANT-SEARCH now says so, and no query word is compared with words outside the vocabulary index.
