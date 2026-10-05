# Distribution

Supported CLI and agent installation paths share the same native executable.

:::mara requirement REQ-SCRIPT-FREE-NPM-DISTRIBUTION
:mid: 01M3HHZSB0M2NBM3D9J78GPZ8A
:title: Run native Mara through script-free npm packages
:status: accepted
:kind: constraint
:derives_from: SCN-INSTALL-DISTRIBUTED-MARA

Install and launch @convesoft/mara without npm lifecycle scripts or a Rust toolchain. The dispatcher and native packages use the exact Cargo workspace version. Support Apple Silicon (arm64) macOS and x64 and arm64 GNU Linux compatible with the Ubuntu 22.04 glibc build baseline. Intel (x64) macOS, Windows and musl Linux are unsupported. Missing or unsupported native packages fail with actionable diagnostics; installation does not download or compile executables through scripts.

The native IPC foundation follows [[DES-LOCAL-DAEMON-PROTOCOL]] and uses
Unix sockets with Linux/macOS peer-user checks. It does not expand this platform
matrix; execution on Linux alone does not certify the macOS host. Lifecycle
commands and package changes remain separate deliverables.

The main package contains its CLI dispatcher, matching standalone Mara skill, README and license texts. Separate ESM library entries, declarations and bundled worker/WASM follow [[DES-INTEGRATION-ARTIFACTS]]. Native packages contain the compiled executable, matching .node addon and public documentation/licenses. Node 24 or newer is required; browser-only library use does not require native optional packages. Preserve CLI arguments, inherited standard streams, exit status and termination signals. [[implemented_by:code:npm/mara.cjs]] [[implemented_by:code:scripts/package-npm.mjs]]
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

The main package exposes bin/mara.cjs and requires Node.js 24 or newer. Exact-version optionalDependencies select @convesoft/mara-linux-x64-gnu, @convesoft/mara-linux-arm64-gnu or @convesoft/mara-darwin-arm64 using npm os, cpu and Linux libc constraints. Each platform package exposes bin/mara and bin/mara.node internally; the Node loader uses the addon without spawning the CLI. The dispatcher resolves the matching installed package from process.platform and process.arch, spawns its executable with inherited streams, and forwards arguments, SIGINT, SIGTERM and SIGHUP.

scripts/package-npm.mjs derives all manifests from [workspace.package].version and copies the source skill into skills/mara/SKILL.md. `platform TARGET BINARY ADDON OUTPUT` requires both prebuilt host artifacts; `main OUTPUT BROWSER_ASSETS` includes all library entries/assets for CI/release. Package generation uses an explicit disposable output directory. It does not run a build, fetch a runtime or publish packages. [[implemented_by:code:npm/mara.cjs]] [[implemented_by:code:scripts/package-npm.mjs]]
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

Build the candidate CLI/addon and browser assets, then run scripts/smoke-npm.sh with those three input paths. Generate and pack the main and host-native packages in disposable storage. Inspect the real npm pack file inventory against the intended dispatcher, skill, library/declaration/worker/WASM, CLI/addon, README, license and manifest files. Require one workspace version, exact native optional-dependency versions and no lifecycle scripts. Install the local tarballs with --ignore-scripts into a disposable project and isolated npm cache.

Run the installed CLI and real stdio MCP server, including bound/unbound project selection, all templates, schema guidance, item/relation authoring, validation and narrative-to-requirement-to-verification navigation. Compare actual CLI/MCP results, consume bounded continuation, reconstruct Unicode content and reject stale cursors. With only Node discoverable on PATH, require the installed dispatcher to run its bundled binary. Compare the installed skill bytes with source. Exercise unsupported schema formats and missing guidance without source writes, then repair the schema in place while preserving item identities, repeated fields, relations and project settings.

Inspect native manifest generation for all three supported targets. Follow [[VER-INSTALLED-JAVASCRIPT-CLIENT]] for the complete installed addon, compiled TypeScript and Chromium HTTP gates. Execute native smoke on each corresponding host before release; one host run proves only that host. Review explicit MCP and separate npx skills setup guidance against the packaged files. Record the checked revision, host, artifact contents and actual workflow results. Package inspection alone does not establish runtime behavior or publication. [[implemented_by:code:scripts/smoke-npm.sh]]
:::
