# Release preparation and public guidance

Release acceptance applies to a captured revision and its actual artifacts.

:::mara requirement REQ-REPRODUCIBLE-PUBLIC-RELEASE
:mid: 01M3HM6K64MMDTPG52QH6KA0S3
:title: Release one verified source revision
:status: accepted
:kind: constraint
:derives_from: SCN-INSTALL-DISTRIBUTED-MARA

Build supported native binaries and npm packages from one captured commit on main. Require formatting, lint, tests, project validation, package inspection and clean-install CLI/MCP smoke on each supported host. Every artifact uses the Cargo workspace version. Generate the committed changelog from Conventional Commit history with git-cliff.

Publication requires approval through the protected GitHub release environment. Create an annotated v<version> tag at the verified commit and a draft GitHub release. Publish native npm packages before the dispatcher, checking any existing package version against its tarball digest on retry. Run the public-registry smoke before publishing the GitHub release. Prereleases use npm next; stable versions use latest. A local host run does not establish other-host or published-release success. [[implemented_by:code:.github/workflows/release.yml]]
:::

:::mara requirement REQ-PUBLIC-REPOSITORY-GUIDANCE
:mid: 01M3HM7STEPDPWSMDYJMTBCG9R
:title: Provide concise public project guidance
:status: accepted
:kind: constraint
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

The repository provides a README with supported installation and primary authoring commands, both license texts, a roadmap of development intentions, private security-reporting guidance and a generated changelog. Public guidance summarizes or links to canonical product contracts; it does not create competing detailed specifications. Roadmap intentions are provisional and do not establish implemented behavior. [[implemented_by:code:README.md]] [[implemented_by:code:ROADMAP.md]] [[implemented_by:code:SECURITY.md]]
:::

:::mara design DES-PROTECTED-RELEASE-WORKFLOW
:mid: 01M3HM9041CZM447W0PEYT8CMP
:title: Separate release verification from approved publication
:status: accepted
:kind: behavior
:satisfies: REQ-REPRODUCIBLE-PUBLIC-RELEASE

Prepare a release through the repository's issue and pull-request flow in [AGENTS.md](../AGENTS.md). Select the workspace version manually, generate CHANGELOG.md with cliff.toml, target main and apply the release label. A merged candidate is eligible for approval. This qualification is a maintainer responsibility: release.yml triggers on a main push changing CHANGELOG.md and does not inspect the pull-request label.

The prepare job captures github.sha, checks version/changelog consistency and an existing tag's target, and runs the required source gates. Four host jobs build and smoke the targets defined by [[DES-NPM-NATIVE-PACKAGES]], then upload temporary native archives and npm tarballs. The publish job depends on those jobs, uses the protected release environment, and alone holds contents-write and npm OIDC permissions. Configure environment protection and npm trusted publishing before using it; the YAML reference to an environment does not prove its server-side protection settings.

Publication assembles the dispatcher and checksums, creates or verifies the captured tag, and creates or updates the draft release assets. Native package publication checks existing registry tarball digests and refuses mismatches, then waits for native packages to become visible. The dispatcher is published or digest-verified last. After it is visible, clean npx execution must report the expected version before GitHub release publication. Retry only the captured revision and byte-identical npm artifacts; publication is staged, so a failed run may already have created a tag, uploaded assets or published some native packages. Inspect the completed stages before retrying. [[implemented_by:code:.github/workflows/release.yml]]
:::

:::mara decision ADR-NPM-NATIVE-DISTRIBUTION
:mid: 01M3HMA67D666SAHFTYJ1VQP0P
:title: Use npm-selected native packages
:status: accepted
:justifies: DES-NPM-NATIVE-PACKAGES

A dispatcher plus native optional dependencies supports one-command npx use without a Rust toolchain or npm lifecycle scripts. An install-time binary downloader would make the primary installation path depend on scripts that enterprise or restricted environments may disable. Supported host boundaries are owned by [[REQ-SCRIPT-FREE-NPM-DISTRIBUTION]]; additional platforms require demonstrated demand and corresponding build and smoke coverage.
:::

:::mara decision ADR-DUAL-LICENSE
:mid: 01M3HMBBXB0PD6FBEQA0DPW9KT
:title: Offer MIT or Apache-2.0 licensing
:status: accepted
:justifies: REQ-PUBLIC-REPOSITORY-GUIDANCE

Recipients may use Mara under either the [MIT License](../LICENSE-MIT) or [Apache License 2.0](../LICENSE-APACHE). This permits broad use while offering Apache's explicit patent grant. Copyright notices name Aliaksei Raketski.
:::

:::mara verification VER-RELEASE-PREPARATION
:mid: 01M3HMCHP5WPFFKWNZB03VN8YF
:title: Inspect release gates and public guidance
:status: accepted
:method: inspection
:verifies: REQ-REPRODUCIBLE-PUBLIC-RELEASE
:verifies: DES-PROTECTED-RELEASE-WORKFLOW
:verifies: REQ-PUBLIC-REPOSITORY-GUIDANCE

Inspect the workflow, packaging scripts, pinned toolchain, Cargo workspace version, changelog configuration and public guidance at the candidate revision. Check captured-revision propagation, all four native host jobs, version derivation, source and packaged-smoke gates, publish-job dependency/permissions/environment, tag-target and npm-digest refusal, native-before-dispatcher visibility and public smoke before GitHub publication. Distinguish maintainer release qualification from automated checks and repository YAML from externally configured protection/trust.

Review README commands, supported installation paths, license references, provisional roadmap and private security guidance for clarity and working repository links. Check shell/JavaScript syntax using their standard runtimes. Run complete schema/project validation and relevant intent/verification matrices, consuming every continuation page. [[VER-NPM-DISTRIBUTION]] owns actual local artifact and CLI/MCP execution.

Record inspection results separately from release execution. For an actual release, require the workflow's successful source gates, corresponding-host artifact smoke, protected approval, matching tag/revision and registry/GitHub publication results. Local inspection cannot certify remote permissions, approval, other-host execution or publication.
:::

:::mara evidence EVD-RELEASE-PREPARATION
:mid: 01M3HMPBPAW4AQYZG3VQBKGJNK
:title: Release source and public guidance inspection passes
:status: accepted
:result: passed
:captured_at: 2026-09-27T14:35:36Z
:subject_revision: d9b9f977e203c7d4a698259e953ada7bb349e986
:evidences: VER-RELEASE-PREPARATION

At `d9b9f977e203c7d4a698259e953ada7bb349e986`, manual inspection of .github/workflows/release.yml, ci.yml, scripts/package-npm.mjs, scripts/smoke-npm.sh, Cargo/toolchain files and cliff.toml confirmed the documented captured-revision gates, four native host jobs, publish dependencies/permissions/environment, tag target and npm digest checks, native-before-dispatcher visibility and public smoke before GitHub publication. Maintainer candidate qualification and externally configured protection/trust are explicitly distinguished from repository automation. The staged retry boundary is documented.

README, roadmap, security guidance and license references were reviewed against current distribution contracts. The generated changelog was inspected as historical release information; no new release was prepared. Installation guidance covers CLI/npm, explicit MCP configuration and separate skill installation. YAML parsing and bash -n passed for all 23 run steps across CI/release workflows; bash -n passed for smoke-npm.sh, and node --check passed for packaging and dispatcher files.

Mara read the complete release section on one page. Candidate schema and project validation completed without diagnostics, one page each. Intent passed both selected requirements on one page; verification passed those requirements and the release design on one page. All continuation pages were consumed. Complete checks used disk-backed temporary storage.

This is source and guidance inspection, not workflow execution. It does not establish remote environment protection, npm trust, approval, native execution on other hosts or publication. Actual packaged host execution is recorded separately at its tested revision.
:::
