# Distribution

## Requirements

---

<a id="01M7BF6GWPMWDV4VT1BA7NEZYN"></a>

### Install and run without a toolchain

| Type | Requirement |
| --- | --- |
| ID | REQ-RUN-WITHOUT-TOOLCHAIN |
| Status | Accepted |
| Satisfies | [NEED-TOOLCHAIN-FREE-INSTALL](product.mara.md#01M7BF6H8C2X09EX518DVSWDCE) |

`npx @convesoft/mara@<version>` runs the CLI, rule evaluation included, with no Rust toolchain, no C++ compiler, no separately installed Soufflé and no npm lifecycle script.

---

## Design

---

<a id="01M7BF6GWZ72PGJNA3VQNZMDC8"></a>

### Packages

| Type | Design |
| --- | --- |
| ID | DES-PACKAGES |
| Status | Accepted |
| Satisfies | [REQ-RUN-WITHOUT-TOOLCHAIN](#01M7BF6GWPMWDV4VT1BA7NEZYN) |

- An npm dispatcher package selects a prebuilt native package for the platform.
- A native package holds the CLI binary, the Node addon and the `souffle` binary. The browser package holds the engine and Soufflé WASM modules.
- Every package carries the same version.
- Platforms: Linux x64 and arm64 (glibc), macOS arm64.
- Node 24 or later.

---

<a id="01M7BF6GX9WNFK70C6H1RBQJ5R"></a>

### Bundled Soufflé

| Type | Design |
| --- | --- |
| ID | DES-SOUFFLE-BUILD |
| Status | Accepted |
| Satisfies | [REQ-RUN-WITHOUT-TOOLCHAIN](#01M7BF6GWPMWDV4VT1BA7NEZYN) |

- CI builds one pinned Soufflé release for each platform: natively with a static C++ runtime and without OpenMP, and with Emscripten for the browser.
- Soufflé 2.5 needs a one-line patch to build with Emscripten's Clang: in `src/interpreter/Index.h:423`, `data = src.data;` becomes `data = src.data.load();`. Upstream `master` no longer has that line.
- Soufflé's licence is UPL-1.0.

---

## Open questions

- The Soufflé build for macOS arm64 was never tried.
- Whether alpha slices are published to npm under the `next` tag.
