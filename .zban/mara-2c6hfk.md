---
title: The npm name of the browser package, and where the Node library is imported from
status: todo
type: task
tags:
  - maintainer
  - question
related:
  - mara-9a1zjp
created_at: 2026-11-25T10:18:35Z
---

Goal: the maintainer names the browser package of `0.4.0-rc.2` before anything packs or publishes it, and confirms where a Node application imports the library from.

Context: `DES-PACKAGES` (docs/distribution.mara.md) lists "the browser package, with the engine and Soufflé WASM modules" from 0.4.0-rc.2 without a name; it is the only package of the table that is not on npm since 0.3.0. A new package needs its name on npm and a Trusted Publisher entry before the release workflow can publish it, both the maintainer's. `REQ-NODE-LIBRARY` and `VER-NODE-LIBRARY` (docs/hosts/) say a Node application "imports the package by its name"; `ADR-NODE-NATIVE-ADDON` ships the addon in the native packages; `DES-PACKAGES` lists the dispatcher's files and none of them is a library. 0.3.0 shipped `browser.mjs`, `node.mjs`, `client.mjs` and `http.mjs` in one npm folder.

1. The browser package's name:
   - `@convesoft/mara-browser` (recommended; provisional until answered): the docs' own word for it, beside `@convesoft/mara-linux-x64-gnu`. mara-jptfbq builds the package under this name as a private package, written in its manifest only.
   - `@convesoft/mara-wasm`, after its contents.
   - No separate package: the WASM modules and the browser client in `@convesoft/mara`. It contradicts `DES-PACKAGES` and puts megabytes of WASM into every install of the CLI.
2. The Node library:
   - `import … from "@convesoft/mara"` (recommended; provisional until answered): the dispatcher gains `exports` with the typed ESM client, which loads the addon from the installed native package as the dispatcher finds `bin/mara`. No new package; `DES-PACKAGES` changes to list the client's files.
   - A package of its own for the client. A second name to install and to keep at the same version.

Until answered: workers build under the recommended names and publish nothing. The issue that packs the browser package waits for this answer.

- [ ] The maintainer has answered both
