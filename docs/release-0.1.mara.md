# Stable 0.1 release

Release preparation owned by [MARA-42](https://linear.app/convesoft/issue/MARA-42/reconcile-the-roadmap-and-release-010).
This document separates the stable runtime contract from the accepted
[0.2 plans](guided-authoring.mara.md).

:::mara design DES-STABLE-010
:mid: 01M23EGWDTTW3J38YADNK97Q0Y
:title: Release the existing single-project workflow as stable 0.1
:satisfies: REQ-REPRODUCIBLE-PUBLIC-RELEASE
:satisfies: REQ-PUBLIC-REPOSITORY-GUIDANCE

Stable 0.1.0 retains the runtime capabilities published in 0.1.0-alpha.3:
project initialization and schema inspection, validation, identity-preserving
item authoring, typed relation mutation, and bounded item search/get/related
through CLI and MCP. Release changes comprise version metadata, public guidance,
canonical documentation, and generated changelog; no 0.2 implementation enters
this release.

Schema and project configuration retain format 1. Existing alpha.3 documents,
IDs/MIDs, custom schemas, command names, and response contracts remain usable.
The new schema guidance requirements, Markdown container/discovery graph,
top-level search/get/related commands, and discovery response format belong to
0.2. Do not apply its migration guide to 0.1.

Known limitations remain one project per operation, item-only discovery and
reading, caller-controlled direct typed relations, and no code-symbol
traceability. Narrative requires filesystem tools. Supported native targets
remain glibc Linux and macOS, x64 and arm64; Windows and musl are unsupported.
Complete-plugin compatibility remains optional under
[[REQ-AGENT-INSTALLATION-MODES]].

Before publication, verify formatting, clippy, all-target tests, and corpus
validation; inspect generated package versions and contents; run clean-install
CLI and stdio MCP workflows without Rust on PATH. The protected workflow must
verify all four target artifacts from the exact merged main commit before its
publication approval. Local host checks alone do not establish other-platform
or public-registry verification.

Use one PR for MARA-42 and the workflow in [[DES-PROTECTED-RELEASE-WORKFLOW]].
Generate the changelog with git-cliff against the intended squash history,
using the final PR title as its Conventional Commit message. Verify identical
regeneration for that projected history before merge and the actual main
revision in the release workflow. Do not publish or mark release completion
until tag, package, registry, and GitHub release evidence exists.
:::

:::mara decision ADR-STABLE-010-PATH
:mid: 01M23EGWE287FEC4XZ630B9909
:title: Promote the verified alpha.3 workflow through one stable release PR
:justifies: DES-STABLE-010

Prepare stable 0.1.0 directly from the published alpha.3 feature set through
the single release PR owned by MARA-42. Replace the earlier planned sequence
of separately published beta and RC versions with explicit readiness checks
on the candidate and its artifacts under [[DES-STABLE-010]].

The remaining work is documentation reconciliation and release verification,
not another feature increment. Separate prerelease names do not provide
additional evidence by themselves; retain the protected publication approval
and every release check. Future 0.2 behavior stays documented as future scope.
:::

:::mara evidence EVD-STABLE-010-LOCAL
:mid: 01M23EJF0Z9Q71BFDQA00D19DN
:title: Verify the local stable 0.1 candidate

Local Linux x64 verification for [[DES-STABLE-010]] with workspace version
0.1.0. Runtime source, dependencies, tests, and packaging scripts are unchanged
from published alpha.3/main commit
`0ef14bdcc64122c2bbb1177c60a40472617b8e81`; the candidate changes release
metadata and documentation.

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed. |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed. |
| `cargo test --locked --all-targets` | 165 passed; two subprocess-helper entry points ignored by the parent run. |
| `cargo run --locked --quiet -- --format json project validate` | Valid, no diagnostics. Revalidated final documentation with the release binary. |
| `cargo build --locked --release` | Passed; executable reports 0.1.0. |
| `env PATH=/usr/bin:/bin bash scripts/smoke-npm.sh target/release/mara` | Passed; neither cargo nor rustc was available on that PATH. |
| Clean npm installation | Host native package and dispatcher both 0.1.0; install scripts disabled; CLI init/validate, bound MCP handshake, and unbound MCP project validation passed. |

This is local candidate evidence, not publication or all-platform evidence.
The protected workflow must still verify the exact merged main revision and
all target artifacts. Merge, annotated tag, stable npm publication and latest
tags, public-registry checks, and the final GitHub release remain pending.
:::
