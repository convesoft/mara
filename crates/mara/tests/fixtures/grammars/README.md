# Grammar fixtures

Official release files of tree-sitter grammars, unchanged, for VER-GRAMMAR-MARKERS ([code link verification](../../../../../docs/code/verification.mara.md)). Each `.wasm` is the release asset; each `tags.scm` is `queries/tags.scm` from the release's `tree-sitter-<language>.tar.gz`. Each directory keeps the grammar's MIT licence as `LICENSE`.

| File | Source | SHA-256 |
| --- | --- | --- |
| `rust/tree-sitter-rust.wasm` | <https://github.com/tree-sitter/tree-sitter-rust/releases/tag/v0.24.2> | `24c89bd9252255e4aebbcbd7d2d308bd92c86dd95a130fdc80efa49577b8d738` |
| `rust/tags.scm` | same release, `tree-sitter-rust.tar.gz` (SHA-256 `953d7a7122cc004d3ad516ae193dde408e633c2da59356ba67e713f3e52d73fc`) | `f22867fdebde5cb091861c08d34690dc2540f4318068bf81be9f6b0d348ab8c1` |
| `python/tree-sitter-python.wasm` | <https://github.com/tree-sitter/tree-sitter-python/releases/tag/v0.25.0> | `16108b50df4ee9a30168794252ab55e7c93bfc5765d7fa0aa3e335752c515f47` |
| `python/tags.scm` | same release, `tree-sitter-python.tar.gz` (SHA-256 `7bce887eb2f33e94bf74a69645cf5138d4096720e54fd3269a6124c06b93c584`) | `d0f3e577878167bfabc30e526e497bce58d1699b9bcabf8ab3a50698efb5ca3e` |

The release assets' SHA-256 digests match those GitHub publishes for them. The tests compute each grammar's pin checksum ([DES-GRAMMAR-ASSETS](../../../../../docs/code/index.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)) from these files.
