# Distribution

Supported CLI and agent installation paths share the same native executable.

:::mara requirement REQ-SCRIPT-FREE-NPM-DISTRIBUTION
:mid: 01M3HHZSB0M2NBM3D9J78GPZ8A
:title: Run native Mara through script-free npm packages
:status: accepted
:kind: constraint
:derives_from: SCN-INSTALL-DISTRIBUTED-MARA

Install and launch @convesoft/mara without npm lifecycle scripts or a Rust toolchain. The dispatcher and native packages use the exact Cargo workspace version. Support x64 and arm64 macOS, and x64 and arm64 GNU Linux compatible with the Ubuntu 22.04 glibc build baseline. Windows and musl Linux are unsupported. Missing or unsupported native packages fail with actionable diagnostics; installation does not download or compile executables through scripts.

The main package contains its CLI dispatcher, matching standalone Mara skill, README and license texts. Native packages contain the compiled executable and public documentation/licenses. Preserve CLI arguments, inherited standard streams, exit status and termination signals. [[implemented_by:code:npm/mara.cjs]] [[implemented_by:code:scripts/package-npm.mjs]]
:::

:::mara requirement REQ-AGENT-INSTALLATION-MODES
:mid: 01M3HJ05KD1FGPVNHSQCJYH58B
:title: Configure MCP and install the standalone skill separately
:status: accepted
:kind: functional
:derives_from: SCN-ONBOARD-MARA-AGENT

Configure an installed executable or exact npm package version as a stdio MCP server. Project selection follows [[DES-OPERATION-PROJECT-CONTEXT]]. Install the matching standalone Mara skill separately through npx skills from a checkout or extracted npm package; select the intended client and scope. Skill installation does not install the executable. CLI fallback reuses the configured MCP launcher and exact version. Mara does not modify the project's AGENTS.md during onboarding.

The [README](../README.md#configure-an-agent) gives the supported setup commands; the [skill](../skills/mara/SKILL.md) owns the agent authoring workflow.
:::

:::mara design DES-NPM-NATIVE-PACKAGES
:mid: 01M3HJ0HSHQY5TEY3717A8BFEE
:title: Dispatch to an npm-selected native package
:status: accepted
:kind: interface
:satisfies: REQ-SCRIPT-FREE-NPM-DISTRIBUTION

The main package exposes bin/mara.cjs and requires Node.js 18 or newer. Exact-version optionalDependencies select @convesoft/mara-linux-x64-gnu, @convesoft/mara-linux-arm64-gnu, @convesoft/mara-darwin-x64 or @convesoft/mara-darwin-arm64 using npm os, cpu and Linux libc constraints. Each platform package exposes bin/mara internally. The dispatcher resolves the matching installed package from process.platform and process.arch, spawns its executable with inherited streams, and forwards arguments, SIGINT, SIGTERM and SIGHUP.

scripts/package-npm.mjs derives all manifests from [workspace.package].version and copies the source skill into skills/mara/SKILL.md. Package generation uses an explicit disposable output directory. It does not run a build, fetch a runtime or publish packages. [[implemented_by:code:npm/mara.cjs]] [[implemented_by:code:scripts/package-npm.mjs]]
:::

:::mara verification VER-NPM-DISTRIBUTION
:mid: 01M3HJ0XZVT1RTY03A9FWJCNBT
:title: Verify installed CLI, skill and MCP packages
:status: accepted
:method: test
:verifies: REQ-SCRIPT-FREE-NPM-DISTRIBUTION
:verifies: REQ-AGENT-INSTALLATION-MODES
:verifies: DES-NPM-NATIVE-PACKAGES
:validates: SCN-INSTALL-DISTRIBUTED-MARA
:validates: SCN-ONBOARD-MARA-AGENT

Build the candidate and run scripts/smoke-npm.sh with its executable path. Generate and pack the main and host-native packages in disposable storage. Inspect the real npm pack file inventory against the intended dispatcher, skill, README, license and manifest files. Require one workspace version, exact native optional-dependency versions and no lifecycle scripts. Install the local tarballs with --ignore-scripts into a disposable project and isolated npm cache.

Run the installed CLI and real stdio MCP server, including bound/unbound project selection, all templates, schema guidance, item/relation authoring, validation and narrative-to-requirement-to-verification navigation. Compare actual CLI/MCP results, consume bounded continuation, reconstruct Unicode content and reject stale cursors. With only Node discoverable on PATH, require the installed dispatcher to run its bundled binary. Compare the installed skill bytes with source. Exercise unsupported schema formats and missing guidance without source writes, then repair the schema in place while preserving item identities, repeated fields, relations and project settings.

Inspect native manifest generation for all four supported targets. Execute native smoke on each corresponding host before release; one host run proves only that host. Review explicit MCP and separate npx skills setup guidance against the packaged files. Record the checked revision, host, artifact contents and actual workflow results. Package inspection alone does not establish runtime behavior or publication. [[implemented_by:code:scripts/smoke-npm.sh]]
:::

:::mara evidence EVD-NPM-DISTRIBUTION
:mid: 01M3HJ8PWWCNKV9PTPFS1R445D
:title: Packaged CLI, MCP and standalone skill pass the Linux x64 workflow
:status: accepted
:result: passed
:captured_at: 2026-09-27T13:53:13Z
:subject_revision: c9e3134eace6c559798ae1d8e2fcd89c2b16b85e
:evidences: VER-NPM-DISTRIBUTION

At subject revision `c9e3134eace6c559798ae1d8e2fcd89c2b16b85e`, scripts/smoke-npm.sh completed against the real candidate executable on Linux x86_64 with glibc 2.44, Node.js 26.7.0 and npm 11.19.0. The dispatcher and native package reported the Cargo workspace version 0.3.0-alpha.3. Packaging, cache, Git configuration and test projects used disposable storage; installation used local tarballs, offline mode and --ignore-scripts.

The actual npm pack inventory for the main package was exactly bin/mara.cjs, skills/mara/SKILL.md, README.md, LICENSE-MIT, LICENSE-APACHE and package.json. The native package inventory was exactly bin/mara, README.md, both licenses and package.json. Assertions checked all four exact-version native optional dependencies and the absence of lifecycle scripts. Installed skill bytes matched the source skill.

The installed CLI and real stdio MCP server passed version, initialization, explicit/bound and per-call/unbound project selection, minimal/empty/engineering templates, schema guidance and real item/relation authoring. With only Node on PATH, the packaged native executable completed narrative-to-item-to-verification navigation, cross-document links/backlinks and sibling navigation, every search/related continuation page, exact bounded Unicode block/section/document reconstruction and stale cursor/handle rejection followed by rediscovery. Custom-schema format and guidance rejection preserved source; in-place repair preserved declarations, repeated fields, relationships, IDs/MIDs and project settings, followed by successful MCP authoring.

All four platform manifests were generated and inspected for names, version, os/cpu/libc constraints, files and lack of lifecycle scripts using an explicitly non-executable fixture payload. This metadata check does not execute other architectures. Native runtime execution here covers Linux x64 only; other supported hosts and protected release publication require their own checks.

JavaScript and shell syntax checks and git diff --check passed. Manual review covered current setup guidance, exact artifact ownership, retained dispatcher behavior and the matching standalone skill path. Candidate schema/project validation completed with no diagnostics. Selected intent passed two roots over 1 page(s), realization two roots over 1 page(s), and verification three roots over 1 page(s); every continuation page was consumed. Exact-revision execution is checked after recording this result. Rust application source and the native dispatcher are unchanged from the verified interface implementation; this result records packaged workflows, not a new full Rust regression run or a published release.
:::
