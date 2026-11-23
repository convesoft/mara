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

On each native platform that [DES-PACKAGES](#01M7BF6GWZ72PGJNA3VQNZMDC8) lists for a published version, `npx @convesoft/mara@<version>` runs the CLI, the MCP server, the daemon and rule evaluation with no Rust toolchain, no C++ compiler, no separately installed Soufflé and no npm lifecycle script.

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

Versions:

- The slices `0.4.0-alpha.1` to `0.4.0-alpha.5` are Git tags only.
- The prereleases of 0.4.0 are published to npm under the dist-tag `next`: `0.4.0-rc.1`, the CLI, MCP server, daemon and rule evaluation on Linux (mara-c8scn7), then `0.4.0-rc.2`, the slice that these documents call 0.4.0-alpha.6, which adds the Node library and the browser (mara-xzfeyj).
- `latest` stays 0.3.0 on every package that 0.3.0 published until 0.4.0 is published under it (mara-chjvk9).
- Every package of a version carries the workspace version, the version of the package `mara` that `cargo metadata --no-deps` reports, which the release workflow publishes ([DES-RELEASE-WORKFLOW](#01M82C8WTK3M2A69XWRGNNQ0SF)). It has no build metadata, which npm drops; packaging fails on a version with build metadata.

Packages, each on npm since 0.3.0 but the browser package:

| Package | Platform | From |
| --- | --- | --- |
| `@convesoft/mara`, the dispatcher | Any | 0.4.0-rc.1 |
| `@convesoft/mara-linux-x64-gnu` | Linux x64, glibc | 0.4.0-rc.1 |
| `@convesoft/mara-linux-arm64-gnu` | Linux arm64, glibc | 0.4.0-rc.1 |
| `@convesoft/mara-darwin-arm64` | macOS arm64 | Before 0.4.0 (mara-ag4ees) |
| The browser package, with the engine and Soufflé WASM modules | Any | 0.4.0-rc.2 |

- The dispatcher holds `package.json`, `bin/mara.cjs`, `README.md`, `LICENSE-MIT` and `LICENSE-APACHE`. Its manifest declares `bin` `mara`, `engines.node` `>=24`, every native package that the version publishes as an optional dependency at exactly its own version, and no `scripts` and no `dependencies`.
- A native package holds `package.json`, `bin/mara` and `bin/souffle`, both mode 0755, `LICENSE-MIT`, `LICENSE-APACHE`, Soufflé's licence as `LICENSE-SOUFFLE`, and `THIRD-PARTY-LICENSES` with the licence of each crate compiled into that `mara`. Its manifest declares `os`, `cpu`, `libc` `["glibc"]` on Linux and the licence `(MIT OR Apache-2.0) AND UPL-1.0`, and no `bin` and no `scripts`. From 0.4.0-rc.2 it also holds the Node addon.
- `THIRD-PARTY-LICENSES` holds the licence files of every crate that `cargo metadata` resolves for the host as a normal or build dependency of `mara`, transitively, without the workspace's crates; features are resolved for the whole workspace, so the list may hold more crates than the build and never fewer. A crate that ships no licence file takes those of the crate of its upstream repository that the packaging script's table names for it, with the same declared licence; packaging fails, naming the crate, for any other.
- The dispatcher's table of platforms is the one list of native packages: the packaging script reads it for the dispatcher's optional dependencies and each native package's `os`, `cpu` and `libc`.
- Each packed file list is checked by name and mode before anything is published: a package holds exactly the files above, each once, and packaging fails otherwise.
- Platforms: Linux x64 and arm64 with glibc 2.35 or later and Landlock ([ADR-SANDBOX-REQUIRED](rules/decisions.mara.md#01M7K4XZRS4DDAREPZSWC0WZF7)); macOS arm64 from mara-ag4ees. Node 24 or later.
- The published Linux binaries are built on the GitHub images `ubuntu-22.04` and `ubuntu-22.04-arm`, whose glibc is 2.35 ([ADR-LINUX-BUILD-IMAGE](decisions.mara.md#01M82C8WTKAM5575JF0MN21ZA4)). No `GLIBC_` symbol version that `bin/mara` or `bin/souffle` needs exceeds 2.35.

The dispatcher:

- It selects the native package for `process.platform` and `process.arch`, and runs its `bin/mara`, with no shell, the caller's arguments, environment and standard streams, only when that package is installed for the dispatcher and its version equals the dispatcher's. Installed for the dispatcher means found in a `node_modules` folder on the dispatcher's own ancestor path, as npm, pnpm and `npx` install it, the first in the order of Node's resolution; a package that Node would find only in its global folders or through `NODE_PATH` counts as not installed.
- It forwards `SIGINT`, `SIGTERM` and `SIGHUP` to `mara`, and exits with its exit status. When a signal ends `mara`, the dispatcher ends itself by that signal where Node can re-raise it; for a signal it cannot re-raise, such as `SIGPIPE`, which Node ignores, or `SIGUSR1`, which starts Node's inspector, it exits with the status 128 plus the signal's number.
- It starts no `mara` and exits 1 with a message on stderr in each of these cases:

| Case | The message names |
| --- | --- |
| A platform or architecture without a native package | The platform and architecture, and the supported platforms |
| Linux on which Node reports no glibc version (`process.report.getReport().header.glibcVersionRuntime`), such as musl | That glibc 2.35 or later is required, and the supported platforms |
| Linux with a glibc older than 2.35 | The required and the found version |
| The native package is not installed, or not at the dispatcher's version | The package and the version, and to reinstall `@convesoft/mara` with optional dependencies |

- If the system lacks a required `GLIBC_` version, the dynamic loader names it when starting the native `mara` directly.

---

<a id="01M7BF6GX9WNFK70C6H1RBQJ5R"></a>

### Bundled Soufflé

| Type | Design |
| --- | --- |
| ID | DES-SOUFFLE-BUILD |
| Status | Accepted |
| Satisfies | [REQ-RUN-WITHOUT-TOOLCHAIN](#01M7BF6GWPMWDV4VT1BA7NEZYN) |

- One pinned Soufflé release is built for each platform: natively with a static C++ runtime and without OpenMP, and with Emscripten for the browser (mara-4dr2p2, by 0.4.0-rc.2).
- CI builds it natively on each of its images for the tests ([DES-CI-WORKFLOW](#01M82C8WTKYXH8S92ZJQ4FX73R)). The release workflow builds it again, from source, on each of its build images, and ships that binary ([DES-RELEASE-WORKFLOW](#01M82C8WTK3M2A69XWRGNNQ0SF)).
- Soufflé 2.5 needs a one-line patch to build with Emscripten's Clang: in `src/interpreter/Index.h:423`, `data = src.data;` becomes `data = src.data.load();`. Upstream `master` no longer has that line.
- Soufflé's licence is UPL-1.0.
- [scripts/build-souffle.sh](../scripts/build-souffle.sh) is the native build for the local platform. It pins the release by tag and the SHA-256 of the downloaded archive, checked before anything is built, and fails when the binary needs a shared C++ runtime or OpenMP or fails a smoke run of `souffle --no-preprocessor -l ""` on JSON facts with no compiler on `PATH`.
- The binary is `target/souffle-<version>/souffle` under the workspace root, with Soufflé's `LICENSE` beside it; the version is 2.5. A rerun only checks an existing binary. The source and build tree live in `target/souffle-<version>/work/` and are removed after a successful build.
- A test or packaging step reads the binary from that path. A test that needs it and does not find it fails with a message that names `scripts/build-souffle.sh`.
- `mara` runs the `souffle` in its own directory, which is where a native package puts it, else `../souffle-2.5/souffle` from there, which is that path for a binary Cargo built in the same `target/`; never one on `PATH`. Without either, each evaluation is a fault that names both paths and `scripts/build-souffle.sh`.

---

<a id="01M82C8WTKYXH8S92ZJQ4FX73R"></a>

### Continuous integration

| Type | Design |
| --- | --- |
| ID | DES-CI-WORKFLOW |
| Status | Accepted |
| Contributes to | [GOAL-FAST-AT-SCALE](product.mara.md#01M7BF6H5JHS6GZ8CMB98MBTMM) |

- The workflow `.github/workflows/ci.yml` runs on each push to `chore/clear-worktree-for-rewrite` and to `main`, and on each pull request. A newer run of the same pull request cancels the older one; a push run is never cancelled.
- Its permissions are `contents: read`, for every job. It uses no secret and writes nothing: no publication, tag, release, comment or cache entry other than its own CI cache.
- One job per image, `Checks (ubuntu-22.04)` and `Checks (ubuntu-22.04-arm)`, neither stopped by the other's failure. The images are the release's build images ([ADR-LINUX-BUILD-IMAGE](decisions.mara.md#01M82C8WTKAM5575JF0MN21ZA4)), so a commit whose CI passed passed the checks where its binaries are built ([ADR-CI-CHECKS](decisions.mara.md#01M82C8WTKTJW5PRD24K1XRBES)). Each job, in order:

| Step | What it does |
| --- | --- |
| Checkout | The pushed or merged commit, with full history and tags: the conversion tests and the key of `mara:test` read the tag `v0.3.0` ([ADR-NX-RUST-TASKS](decisions.mara.md#01M8094G2DV9JZ35C8VRMFPWYV)) |
| Toolchains | `rustup toolchain install`, which installs what `rust-toolchain.toml` pins; the pnpm of `packageManager`; Node 24, the oldest Node that [DES-PACKAGES](#01M7BF6GWZ72PGJNA3VQNZMDC8) supports; cargo-nextest 0.9.152, the runner of the `test` tasks ([ADR-NX-RUST-TASKS](decisions.mara.md#01M8094G2DV9JZ35C8VRMFPWYV)): the `x86_64` or `aarch64` `unknown-linux-gnu` archive of its GitHub release, for the runner's architecture, checked against the SHA-256 that the workflow pins for it and put on `PATH`. The archives need glibc 2.27. Moving the pin changes the version and both SHA-256 values. No build tools: both images have every tool that `scripts/build-souffle.sh` names, ninja included |
| Dependencies | `pnpm install --frozen-lockfile`. `zban` is the devDependency `@convesoft/zban` at an exact version, so that `cargo-workspace:zban-check` runs the same `zban` in CI and locally; its key runs the `zban` that Nx's command finds first, the one in `node_modules/.bin` ([ADR-NX-RUST-TASKS](decisions.mara.md#01M8094G2DV9JZ35C8VRMFPWYV)). `minimumReleaseAgeExclude` in `pnpm-workspace.yaml` exempts exactly the pinned version of it and its native packages while that release is younger than pnpm's minimum release age; remove those entries once it is older, or when the pin moves |
| Caches restored | Cargo's entry: the directories `target/debug`, `target/release` and `target/wasm32-unknown-unknown`, named one by one, and Cargo's registry, keyed by the prefix `cargo-dirs-`, the image, the architecture and `hashFiles('rust-toolchain.toml', 'Cargo.lock')`, with the newest entry of that prefix, image and architecture as fallback. The rest of `target/` is outside it: `target/souffle-2.5`, nx-rust's stages in `target/.nx-rust-stage`, the tests' scratch directory `target/tmp` and `target/.rustc_info.json`; Nx's cache and workspace data lie in `.nx/`. Then Soufflé's entry: `target/souffle-2.5`, keyed by the image, the architecture and `hashFiles('scripts/build-souffle.sh')`, with no fallback. Soufflé comes only from its own entry, because the script keeps an existing binary that passes its checks |
| Grammar | `cargo run --locked --quiet --bin mara -- --no-daemon code add rust`, which fetches the grammar that `.mara/config.toml` pins into the per-user cache and checks it against the pin ([DES-GRAMMAR-ASSETS](code/index.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)); it is fetched on each run, never cached |
| Checks | `pnpm run checks` with `NX_DAEMON=false`, the same command as the local checks, so that a failure in CI reproduces with it; Nx runs `scripts/build-souffle.sh`, which only checks a restored binary |
| Soufflé saved | After the checks, also when one failed: `target/souffle-2.5` under its key, when no entry was restored and the script left a binary, which it does only after its checks pass. Cargo's entry is saved at the job's end, only when the job passed and no entry of its key exists |
| Clean tree | `git status --porcelain` prints nothing: no check changed a tracked file or left an untracked, non-ignored file |

- The Toolchains and Dependencies steps are the composite action `.github/actions/setup`, which the release workflow runs too ([DES-RELEASE-WORKFLOW](#01M82C8WTK3M2A69XWRGNNQ0SF)); it restores no cache.
- Nx's cache directory is never saved or restored between runs: each run executes every target, and reuses a result only within the run.
- Which tests a `test` target runs in CI, the full suite or those a change affects, is what [ADR-NX-RUST-TASKS](decisions.mara.md#01M8094G2DV9JZ35C8VRMFPWYV) gives for the event; where it gives no mode, the full suite.
- `mara:test:performance` runs on GitHub's shared runners, as the last `nx` command of `pnpm run checks`, so that no other check of the job runs beside it. Its result counts like any check's, with no automatic retry: [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) defines it for shared machines.
- Not part of CI: the nightly 64× run of the counter test (`--ignored`), the `hyperfine` budgets of [VER-TIME-BUDGETS](verification.mara.md#01M7BF6HH06WEJ67P04H22BM66) and the Emscripten build of Soufflé. A platform of [DES-PACKAGES](#01M7BF6GWZ72PGJNA3VQNZMDC8) without a native package has no job.
- CI adds no Nx target and builds no package.

---

<a id="01M82C8WTK3M2A69XWRGNNQ0SF"></a>

### Release workflow

| Type | Design |
| --- | --- |
| ID | DES-RELEASE-WORKFLOW |
| Status | Accepted |
| Satisfies | [REQ-RUN-WITHOUT-TOOLCHAIN](#01M7BF6GWPMWDV4VT1BA7NEZYN) |

- The workflow `.github/workflows/release.yml` starts only on `workflow_dispatch`, with the inputs `dist_tag`, a string, default `next`, and `dry_run`, a boolean, default true. No push and no pull request starts it ([ADR-DISPATCHED-RELEASE](decisions.mara.md#01M82C8WTKPTQCXJQT024J3GP5)).
- A dispatch is a dry run unless `dry_run` is off. It uses `dist_tag` as given, so that an empty one is refused. Before a release, one dry dispatch on `main` passes on the commit that is then dispatched with the dry run off.
- Runs of the same ref queue, in order (GitHub's concurrency `queue: max`).
- The version is set by hand before a release: one commit changes `[workspace.package].version` in `Cargo.toml` and the workspace crates' entries in `Cargo.lock`. The workflow reads it with `cargo metadata` and changes no file of the repository. `nx release version` and `nx release publish` stay unused ([ADR-DISPATCHED-RELEASE](decisions.mara.md#01M82C8WTKPTQCXJQT024J3GP5)).
- Every job checks out the commit that Prepare records. Every published file, `souffle` included, is built in the run on that commit: no CI cache is restored, and the Nx cache directory starts empty.
- The workflow runs no checks. A release is dispatched on a commit of `main`, whose checks CI ran on the same images ([DES-CI-WORKFLOW](#01M82C8WTKYXH8S92ZJQ4FX73R)). Prepare does not read CI's result for the commit: `main`'s required checks hold it ([ADR-DISPATCHED-RELEASE](decisions.mara.md#01M82C8WTKPTQCXJQT024J3GP5)).
- The `release` block of `nx.json` configures `nx release changelog` alone, after zban's and nx-rust's: the project `mara`, whose version the packages carry; Conventional Commits without scopes; tags `v{version}`; one workspace changelog, written to no file and published as a GitHub release (`createRelease: "github"`), and no project changelogs; the changelog's git settings tag and push, and never commit.
- The notes list the Conventional Commits from the previous release's tag to the recorded commit, under Nx's headings, with links to the commits and pull requests and the authors' GitHub names. Nx takes as the previous release a tag `v<version>` that the commit contains. With Nx's default `strictPreid`, Nx prefers the latest matching prerelease tag if its base version is newer than the latest stable tag. Otherwise it uses the latest stable tag. Stable releases use stable tags. So `0.4.0-rc.1` starts at `v0.3.0`, past the `v0.4.0-alpha.N` tags, `0.4.0-rc.2` at `v0.4.0-rc.1`, and `0.4.0` at `v0.3.0`. `automaticFromRef` starts at the first commit when no tag matches.
- To render the authors' GitHub names, Nx sends the commit authors' email addresses to ungh.cc and then to GitHub's user search; `release.changelog.workspaceChangelog.renderOptions.applyUsernameToAuthors` set to false turns that lookup off. It stays at Nx's default, as in zban and nx-rust.
- To look a version's notes up again once its tag exists, delete the local copy of the tag first (`git tag -d`): with it, Nx starts the range at that tag and finds no commits.
- Publish and Tag run in the GitHub environment `release`, whose required reviewer is the maintainer, which administrators cannot bypass, and whose deployment branch policy allows only `main`: no run publishes or tags without the maintainer's approval. The workflow neither creates nor changes the environment.
- The workflow's permissions are `contents: read`; a job that needs more says so below. Its jobs, in order:

| Job | Runs on | Permissions | What it does |
| --- | --- | --- | --- |
| Prepare | `ubuntu-24.04` | `contents: read` | Records the commit. Reads the version and refuses one that is not SemVer. Refuses a dist-tag that does not match `^[a-z][a-z0-9._-]*$`, and the dist-tag `latest` for a version with a prerelease part. Refuses a dispatch without the dry run on another ref than `refs/heads/main`. Refuses a version whose tag `v<version>` exists or that npm has for any of the packages. Records which packages npm has the version of |
| Pack dispatcher | `ubuntu-24.04` | `contents: read` | After Prepare: the checkout, toolchains (cargo-nextest included) and dependencies steps of [DES-CI-WORKFLOW](#01M82C8WTKYXH8S92ZJQ4FX73R), so that the pinned Rust toolchain is installed although nothing is compiled, because nx-rust builds the project graph with cargo and the packaging script reads the version with `cargo metadata`; then the dispatcher packed by `pack-dispatcher` |
| Build | `ubuntu-22.04` and `ubuntu-22.04-arm`, one job each, neither stopped by the other's failure | `contents: read` | After Pack dispatcher: the setup steps of Pack dispatcher; the native package packed by `pack-native`, which builds `mara` and, through `cargo-workspace:souffle`, Soufflé with `scripts/build-souffle.sh` from source, beside `mara`'s build (on 2026-11-23, 8 min 47 s on `ubuntu-22.04` and 6 min 44 s on `ubuntu-22.04-arm`, of Build jobs of 9 min 29 s and 7 min 23 s); the install verification on the packed dispatcher and native package, whose analysis checks that no `GLIBC_` symbol version of its `mara` or `souffle` exceeds 2.35 ([VER-INSTALL-WITHOUT-TOOLCHAIN](verification.mara.md#01M82C8WTK2PAA1SBEWCAJ4ND8)); then records the SHA-256 of the two tarballs it verified, in the artifact that carries its native tarball |
| Dry run | `ubuntu-24.04` | `contents: read` | Every dry run: the checkout with every tag and the setup steps of Pack dispatcher; checks the tarballs (below), then `npm publish --dry-run --access public --tag <dist_tag>` of each, native packages first. A package whose version npm has by then, published after Prepare's check, is left out with a warning, because npm refuses even a dry run of a published version. The job reads the registry for each package right before its dry run, and again when npm refuses it: a version that npm has by then is left out with the warning, any other refusal fails the job. Then `nx release changelog <version> --dry-run --verbose` with Tag's push arguments prints the notes and the tag, push and release that Tag would make; it reads the tag's release through the API with the job's token |
| Publish | `ubuntu-24.04`, environment `release` | `contents: read`, `id-token: write` | A dispatch without the dry run: records each package's dist-tags before the run's first publication attempt (below); publishes each native package with `npm publish <tarball> --provenance --access public --tag <dist_tag>` through npm Trusted Publishing, with npm 11.5.1 or later and no npm token; waits until npm shows every native package, polling for at most 30 minutes; publishes the dispatcher the same way |
| Registry check | `ubuntu-22.04` and `ubuntu-22.04-arm` | `contents: read` | After Publish: in an empty directory, `npx --yes --ignore-scripts @convesoft/mara@<version> --version` prints `mara <version>`, retried for at most 30 minutes until npm serves it; the install verification's cases but the tarballs' file lists, on `@convesoft/mara@<version>` installed from the registry with `--ignore-scripts`; for each package, `<dist_tag>` names the version and every other dist-tag names what it named before Publish |
| Tag | `ubuntu-24.04`, environment `release` | `contents: write`, used through the job's `GITHUB_TOKEN` | After every registry check: the checkout with every tag and the setup steps of Pack dispatcher; `nx release changelog <version> --verbose --git-push-args="--no-follow-tags refs/tags/v<version>"` as `github-actions[bot]`, which creates the annotated tag `v<version>`, message `v<version>`, on the recorded commit, pushes that tag alone (`--no-follow-tags` overrides the `--follow-tags` that Nx puts first), and creates the GitHub release of the tag, titled `v<version>`, with the notes, marked as a prerelease when the version has a prerelease part; then every package's tarball and their `SHA256SUMS` attached |

- The tarballs travel between jobs as artifacts of the run. Dry run, Publish and Tag run the composite action `.github/actions/release-tarballs`, which fails unless there is exactly one tarball per package, each with the SHA-256 that a Build job recorded for it, and writes their `SHA256SUMS`: every job publishes or attaches only tarballs that an install verification ran on.
- The command `.github/scripts/npm-registry.sh` reads npm's registry for Prepare, Dry run, Publish and Registry check, from a package's abbreviated document, which holds its dist-tags and each version's `dist.shasum`. A version is present when the document lists it with a 40-digit SHA-1, absent when the package's document does not list it or the registry answers 404 with its `{"error":"Not found"}` document. Any other answer, a listed version without a SHA-1 included, is retried 5 times 10 s apart and then fails, never read as absent. Each request stops after 30 s, each `npx` try of Registry check after 5 minutes, so that a stalled request overruns a 30-minute wait by minutes at most.
- The dist-tags that Publish records are an artifact of the run, uploaded before its first `npm publish`. A re-run of Publish keeps the artifact of an earlier attempt and takes no new snapshot; finding none, it records the tags then, since no earlier attempt published. The upload fails when the artifact exists, so a failed download cannot replace it. Registry check compares each package's other dist-tags with that artifact as JSON values, in any order.
- Each `npx` try of Registry check has an empty directory and npm cache, so that an installation that missed a native package npm did not serve yet is not reused.
- A re-run of the failed jobs of the same run continues a partial publication with the same tarballs:
  - Publish skips a package version already on npm whose `dist.shasum` equals the SHA-1 of its tarball, and fails on another; a registry error is retried, and only a version npm reports absent is published.
  - Tag looks the tag up on `origin` and the release through the REST API, where 404 means none, before it runs Nx. An existing release and its notes are kept; a release whose tag `origin` lacks fails the job, so that Nx neither re-creates the tag nor replaces the notes. Tag accepts an existing `v<version>` that names the recorded commit and fails on another. With the tag and without a release, it deletes the local copy of the tag, so that the notes start at the previous release as on the first attempt, and runs `nx release changelog <version> --git-tag=false --git-push=false`, which creates the release alone. Tag runs Nx with empty standard input, so that Nx's prompt after a failed API call fails the job. It attaches each tarball and `SHA256SUMS` unless the release holds an asset of that name in the state `uploaded`, whose SHA-256 (the asset's `digest`, else its download) must equal the run's; an asset in another state is an interrupted upload, which it deletes and attaches again.
- Tag relies on behaviours of Nx 24.0.6 that are not documented options, to check again when Nx is upgraded: with the version's tag present locally, Nx starts the range at that tag; Nx updates the release that exists for the tag; Nx puts `--follow-tags` before the given push arguments.
- The project `mara-npm` in `packages/mara-npm` holds the dispatcher `bin/mara.cjs`, the packaging script `scripts/pack.mjs` and the install verification `scripts/verify-install.mjs`. The Nx project is defined by `project.json`, outside the pnpm workspace.
- The packaging script reads the version, the repository and Cargo's artifact directory with `cargo metadata --no-deps`. It packs with `npm pack --ignore-scripts --offline --no-update-notifier` and an empty npm cache of its own, into a temporary name in `dist/npm` of the workspace, and renames the checked tarball over an earlier one of the same name; a failed pack leaves that earlier tarball as it was.
- The project's Nx targets are not cached, because each writes or checks published files and reads built binaries or the machine's `PATH`:

| Target | Runs | Depends on |
| --- | --- | --- |
| `pack-dispatcher` | The packaging script for the dispatcher alone | Nothing built |
| `pack-native` | Run as `nx run mara-npm:pack-native:release`: the packaging script for the native package of the host's platform, on the `mara` of `mara:build:release`, which it reads from `release/mara` in the artifact directory that `cargo metadata` names, so that `CARGO_TARGET_DIR` and Cargo's configuration apply as they do to the build; with `CARGO_BUILD_TARGET` set it fails. It cannot see a `build.target` set in a Cargo configuration file, which this repository does not set; with one it would read a `release/mara` that the build did not write. It packs the `souffle` and `LICENSE` of `cargo-workspace:souffle` and the repository's licences; it writes `THIRD-PARTY-LICENSES` for that build. Without the configuration it fails before packing, because Nx gives a dependency the configuration of the command, not the target's default | `mara:build:release`, a configuration `release` of the inferred `build` with `profile = "release"`; `cargo-workspace:souffle` |
| `verify-install` | The install verification, on the tarballs in `dist/npm`, or with `--registry=<version>` on that version from the registry | Nothing built |

- Publication, the waits, the registry check and the tag are steps of the workflow, not targets.
- The three targets are not part of `pnpm run checks`, because they need a release build and write tarballs: the release workflow runs them, and anyone can on demand.

---

## Open questions

- Whether the notes of `0.4.0`, which start at `v0.3.0` ([DES-RELEASE-WORKFLOW](#01M82C8WTK3M2A69XWRGNNQ0SF)), leave out the 29 features, fixes and performance changes of the previous implementation between `v0.3.0` and `d7f0e73`, which 0.4.0 does not ship. The notes of `0.4.0-rc.1` list them.
- The Soufflé build for macOS arm64 was never tried.
