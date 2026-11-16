---
title: No operation removes a metadata row whose key the type does not declare
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-6k50w0
created_at: 2026-11-16T15:26:23Z
---

Goal: an agent can remove a row that validation reports as `undeclared-key` without editing the file by hand.

Context: found by the orchestrator on 2026-11-16 while running the alpha.2 epic's "Done when" through the MCP server on a copy of this repository's docs. A hand-written item had the row `| Method | Test |` on a Verification, whose field is `methods`. `project validate` reports `undeclared-key`; `project fix` rightly leaves it; `item update --clear-field method` is refused with `invalid-item` ("type `verification` declares no field `method`: it declares `Methods`"). The only ways out are a hand edit of the row, or deleting and recreating the item, which gives it a new MID. Low priority: operations never write such a row, so it comes only from hand-written text, and a hand edit fixes it. In the backlog under the maintainer's rules of 2026-11-15; nothing here starts without the maintainer.

- [ ] Decide: `--clear-field` accepts a key the item has although the type does not declare it, or the limit is stated in DES-AUTHORING-REQUESTS and the agent guide
