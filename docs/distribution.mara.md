# Distribution and release

This document owns the durable distribution and release contract. Public
installation instructions summarize it in `README.md`.

:::mara scenario SCN-INSTALL-DISTRIBUTED-MARA
:mid: 01M1PXP2KG35JD2VV6SBSXPDQW
:title: Run Mara without a Rust toolchain

On a supported host, a user runs an exact `@convesoft/mara` version through
`npx`. The package runs the native Mara binary with inherited standard streams,
so the same command serves the CLI and long-running stdio MCP workflows. This
advances [[GOAL-UNIFIED-PROJECT-KNOWLEDGE]] and
[[GOAL-BOUNDED-AGENT-CONTEXT]].
:::

:::mara requirement REQ-SCRIPT-FREE-NPM-DISTRIBUTION
:mid: 01M1PXP2KGSGNZEYN88YWF7S2Y
:title: Distribute supported native binaries through script-free npm packages
:derives_from: SCN-INSTALL-DISTRIBUTED-MARA

`@convesoft/mara` must install and launch without npm lifecycle scripts. Every
release publishes the dispatcher and these native packages at one exact
application version:

- `@convesoft/mara-linux-x64-gnu`
- `@convesoft/mara-linux-arm64-gnu`
- `@convesoft/mara-darwin-x64`
- `@convesoft/mara-darwin-arm64`

Linux support requires the GNU target and compatibility with the Ubuntu 22.04
glibc baseline used to build release artifacts. Windows and musl Linux are
unsupported for the first alpha. An unsupported or missing native package must
fail with an actionable diagnostic instead of downloading or building code at
install time.
:::

:::mara requirement REQ-AGENT-INSTALLATION-MODES
:mid: 01M1PXP2KGPHVWQQ8BGR9KDF12
:title: Support manual and optional complete-plugin agent onboarding
:derives_from: SCN-ONBOARD-MARA-AGENT

The supported Codex path registers an installed Mara executable as an MCP server
and installs the Mara skill separately. The Convesoft marketplace may also
offer the complete package as an optional convenience. The two routes expose
the same skill and MCP operations, but complete-plugin compatibility is not a
release gate.
:::

:::mara requirement REQ-REPRODUCIBLE-PUBLIC-RELEASE
:mid: 01M1PXP2KGMG4DYF9JAN85XXMH
:title: Publish one verified release from one approved revision
:derives_from: SCN-INSTALL-DISTRIBUTED-MARA

A release must build all supported binaries and npm packages from one commit on
`main`; run formatting, lint, tests, project validation, package inspection, and
clean-install CLI and MCP smoke tests; and verify that every artifact carries
the Cargo workspace version. The committed changelog must be generated from
Conventional Commit history with `git-cliff`.

Publication requires approval through the protected GitHub `release`
environment. The approved workflow creates an annotated `v<version>` tag at the
verified commit, creates a draft GitHub release, publishes native npm packages
before the dispatcher, verifies any already-published version by tarball digest
on retry, and runs the public-registry smoke test before publishing the GitHub
release. Prereleases use the npm `next` tag; stable releases use `latest`.
:::

:::mara requirement REQ-PUBLIC-REPOSITORY-GUIDANCE
:mid: 01M1PXP2KG4VCF5PT6TTZ6QJ9W
:title: Keep public project and release guidance discoverable

The repository root must provide a concise README, dual-license texts, current
roadmap through 0.3.0 and Later, security reporting guidance, and generated
changelog. These conventional files must link to canonical Mara contracts
instead of duplicating their detailed meaning.
:::

:::mara design DES-NPM-NATIVE-PACKAGES
:mid: 01M1PXP2KG1AN1F4WE281BQ028
:title: Dispatch to an npm-selected native package
:satisfies: REQ-SCRIPT-FREE-NPM-DISTRIBUTION
:satisfies: REQ-PORTABLE-AGENT-ONBOARDING

The script-free `@convesoft/mara` package exposes `mara` through a small Node.js
dispatcher. Its exact-version `optionalDependencies` use npm `os`, `cpu`, and
`libc` selection to install only the matching native package. Each native
package contains the compiled Rust executable. The dispatcher selects the
package from `process.platform` and `process.arch`, resolves its executable,
forwards arguments and standard streams, and mirrors its exit status or signal.

Package manifests and the packaged Agent Plugin version are assembled from
repository templates and the version in `[workspace.package]`; they do not
maintain an independent release version.
:::

:::mara design DES-CODEX-AGENT-DISTRIBUTION
:mid: 01M1PXP2KGCASKQ0HAVTQMK4R4
:title: Distribute manual and optional complete-plugin Codex onboarding
:satisfies: REQ-AGENT-INSTALLATION-MODES

The supported route registers the installed executable through Codex MCP
configuration and installs the Mara skill independently. It does not create or
link a Codex plugin-cache entry. The optional Convesoft marketplace is named
`convesoft` and exposes plugin `mara` from the release channel used for
`@convesoft/mara`: `next` during prereleases and `latest` after the stable
release. Complete-plugin installation remains client-managed convenience
behavior outside automated release verification.
:::

:::mara design DES-PROTECTED-RELEASE-WORKFLOW
:mid: 01M1PXP2KGJBRNZHGGGJJ1GVA4
:title: Build before approval and publish after approval
:satisfies: REQ-REPRODUCIBLE-PUBLIC-RELEASE

A pull request becomes a release candidate only when it targets `main`, updates
the generated `CHANGELOG.md`, carries the `release` label, and is merged. The
`release.yml` workflow then derives the exact Cargo version from the merge
commit. Unprivileged jobs validate that commit, build and smoke-test all target
artifacts, and upload temporary workflow artifacts. The only job with
`contents: write` and npm OIDC permission depends on those jobs and uses the
protected `release` environment.

The release job is retryable only for the captured commit and byte-identical npm
tarballs. It refuses a tag at another commit or an existing package with a
different registry tarball digest. Native packages must become visible in the
public registry before the dispatcher is published, and the dispatcher must
become visible before the final clean `npx` and GitHub release publication
checks. Optional client plugin installation does not participate in the release
transaction.
:::

:::mara decision ADR-NPM-NATIVE-DISTRIBUTION
:mid: 01M1PXP2KGX9Z4Q22NMPF1BQBW
:title: Use npm platform packages instead of an install-time downloader
:justifies: DES-NPM-NATIVE-PACKAGES

Mara uses a dispatcher plus npm-selected native packages because it provides a
one-command `npx` path without a Rust toolchain or lifecycle scripts. An
install-time binary downloader is rejected because blocked or restricted npm
scripts would make the primary installation path unreliable, especially in
enterprise environments.
:::

:::mara decision ADR-CLIENT-MANAGED-PLUGIN-INSTALLATION
:mid: 01M1PXP2KGC95GEMT8MR00DWEN
:title: Keep installed plugin state client-managed
:justifies: DES-CODEX-AGENT-DISTRIBUTION

Do not edit or symlink Codex plugin-cache entries to reuse another Mara package
installation. Codex owns the plugin snapshot's validation, enablement, update,
and removal, while npm owns the exact-version native runtime selected by its
launcher. Users who want complete onboarding accept those managed artifacts;
users who already installed Mara reuse it through MCP configuration and install
only the small skill. This avoids a second native executable without depending
on Codex's internal cache layout.
:::

:::mara decision ADR-FIRST-ALPHA-TARGETS
:mid: 01M1PXP2KGV6WJSBS36FTBXXPM
:title: Limit the first alpha to glibc Linux and macOS
:justifies: REQ-SCRIPT-FREE-NPM-DISTRIBUTION

The first alpha supports x64 and arm64 on glibc Linux and macOS. Windows and
musl Linux remain explicit limitations until real usage justifies their build,
packaging, and verification cost.
:::

:::mara decision ADR-DUAL-LICENSE
:mid: 01M1PXP2KGM980VC45RM6VSEVB
:title: License Mara under MIT or Apache-2.0
:justifies: REQ-PUBLIC-REPOSITORY-GUIDANCE

Recipients may use Mara under either the MIT License or Apache License 2.0.
This preserves permissive use while providing Apache's explicit patent grant.
Copyright notices name Aliaksei Raketski.
:::

:::mara decision ADR-TRUNK-BASED-RELEASES
:mid: 01M1PXP2KG4AF9B3AB69M3HSZ7
:title: Release from main through short-lived issue branches
:justifies: DES-PROTECTED-RELEASE-WORKFLOW

Mara uses `main` as its only long-lived branch. Work uses short-lived Linear
issue branches and one squash-merged pull request per issue. Release preparation
uses the same flow; there are no `develop` or release branches. After the
release-preparation change is merged, the protected workflow tags the exact
validated `main` revision only after deployment approval.
:::
