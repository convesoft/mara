# Decisions

---

<a id="01M7BF6GTYJB4MCBXQSHWFXMYZ"></a>

### Write items as ordinary Markdown

| Type | Decision |
| --- | --- |
| ID | ADR-MARKDOWN-NATIVE-ITEMS |
| Status | Accepted |
| Justifies | [DES-ITEM-STRUCTURE](format.mara.md#01M7BF6GZDTNNW0Z0MX7FEG6ZF) |

- Chosen: an item is an HTML anchor, an ATX heading and a GFM table.
- Rejected: `:::mara` directive blocks. GitHub shows them as raw text, without highlighting, rendering or link navigation.

---

<a id="01M7BF6GVAEPTJR91QAR6QRW24"></a>

### Store the MID in an uppercase HTML anchor

| Type | Decision |
| --- | --- |
| ID | ADR-MID-IN-HTML-ANCHOR |
| Status | Accepted |
| Justifies | [DES-MID-ANCHOR](identity.mara.md#01M7BF6H1MB5RR3RS43B3CJHAR) |

- Chosen: `<a id="<MID>"></a>` is the only place a MID is stored; it is the identity and the link target at once.
- Rejected: heading slugs as link targets. They change with the title.
- Rejected: lowercase MIDs in anchors and links. GitHub already matches fragments case-insensitively ([EVD-GITHUB-FRAGMENT-MATCHING](identity.mara.md#01M7BF6H2MHAVK1Z5HCXR1V289)).

---

<a id="01M7BF6GVK3EZ2G1CYXSEB9XC3"></a>

### Author typed relations only in the source item's table

| Type | Decision |
| --- | --- |
| ID | ADR-RELATIONS-ONLY-IN-TABLE |
| Status | Accepted |
| Justifies | [DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V) |
| Justifies | [REQ-EDIT-RELATIONS](authoring/index.mara.md#01M7BF6GN8DJ66NPBMQK83H9PW) |

- Chosen: one row on the source item, under the forward relation name, holding a Markdown link.
- Rejected: `[[ID]]` and `[[relation:target]]` syntax. It is not Markdown and does not navigate on GitHub.
- Rejected: rows under the inverse name on the target item. One edge would have two spellings.
- Rejected: an inverse name in `relation add`, written as the forward row on the other item. The operation would edit an item it was not pointed at.

---

<a id="01M7CMW6Y0WFD4W6951906MXQY"></a>

### Keep configuration and schema as TOML files under `.mara/`

| Type | Decision |
| --- | --- |
| ID | ADR-TOML-PROJECT-FILES |
| Status | Accepted |
| Justifies | [DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27) |

- Chosen: `.mara/config.toml` and `.mara/schema.toml`, each with its own format version starting at 1.
- Rejected: the 0.3.0 names, `.mara/project.toml` and `.mara/schema.yaml`. YAML needs a second parser, and the format versions would continue at 4.
- Rejected: one `mara.toml` at the project root. A schema edit would change the configuration hash, and `project init` could not write the schema as its own file.

---

<a id="01M7BF6GWEX6QCDC1YGR54P23Z"></a>

### Distribute prebuilt binaries as npm packages

| Type | Decision |
| --- | --- |
| ID | ADR-SCRIPT-FREE-NPM-PACKAGES |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: a dispatcher package with one optional dependency per platform.
- Rejected: an install-time downloader. It depends on lifecycle scripts, which restricted environments disable.

---

<a id="01M7CMW6YNXSQAS7H0ATKG4GH0"></a>

### Keep the 0.3.0 platforms and Node version

| Type | Decision |
| --- | --- |
| ID | ADR-KEEP-PLATFORMS-AND-NODE |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: Linux x64 and arm64 (glibc), macOS arm64 and Node 24, as in 0.3.0.
- Rejected until requested: Windows. It needs a Soufflé MSVC build, a patch for explanations (the explain console throws `No explain/explore provenance on Windows`) and named-pipe IPC for the daemon.
- Rejected until requested: Intel macOS and musl Linux.
- Rejected: Node 22. It reaches end of life in April 2027.

---

<a id="01M7CQ57SQY6M97NEDTA27K5WE"></a>

### Embed Mara in Node through a native addon

| Type | Decision |
| --- | --- |
| ID | ADR-NODE-NATIVE-ADDON |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: an N-API addon over the same Rust host code as the CLI, shipped in the native packages.
- Rejected: the WASM engine with a host written in JavaScript. A second host would have to match the Rust one in file access, hashing, caching and running Soufflé.

---

<a id="01M82C8WTKAM5575JF0MN21ZA4"></a>

### Build the Linux binaries on Ubuntu 22.04

| Type | Decision |
| --- | --- |
| ID | ADR-LINUX-BUILD-IMAGE |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen by the worker of mara-a57pzn on 2026-11-22: the GitHub images `ubuntu-22.04` and `ubuntu-22.04-arm`, whose glibc is 2.35, the images of 0.3.0's release. This preserves 0.3.0's Linux glibc floor.
- Rejected: `ubuntu-24.04`. Its glibc 2.39 refuses Debian 12 (2.36) and Ubuntu 22.04 (2.35).
- CI runs on these images too ([ADR-CI-CHECKS](#01M82C8WTKTJW5PRD24K1XRBES)).
- Rejected: static musl binaries, as zban ships. Soufflé was never built against musl, and the platforms are glibc ([ADR-KEEP-PLATFORMS-AND-NODE](#01M7CMW6YNXSQAS7H0ATKG4GH0)).
- Rejected: a build in an older container, such as a manylinux image. Soufflé needs CMake, Bison, Flex and a C++17 compiler inside it, a second build environment beside CI's.
- Known limit: GitHub keeps at most two generally available Ubuntu images and listed 26.04, 24.04 and 22.04 on 2026-11-22. When it retires `ubuntu-22.04`, the minimum glibc and CI's images change by a new decision.

---

<a id="01M82C8WTKPTQCXJQT024J3GP5"></a>

### Publish from a dispatched workflow that defaults to a dry run

| Type | Decision |
| --- | --- |
| ID | ADR-DISPATCHED-RELEASE |
| Status | Accepted |
| Justifies | [DES-RELEASE-WORKFLOW](distribution.mara.md#01M82C8WTK3M2A69XWRGNNQ0SF) |

- Chosen by the maintainer on 2026-11-22: a `workflow_dispatch` with a dist-tag and a dry run that is the default and the only run an agent starts. The workflow creates the tag, after the packages are on npm; nobody tags by hand.
- Rejected: 0.3.0's publication on each push to `main` that changes `CHANGELOG.md`. Every merge of that file published.
- Chosen by the maintainer on 2026-11-22: publication only from `main`, in the environment `release`, whose branch policy is `main`; `main` is fast-forwarded to the integration branch after a dry run passes on its tip (mara-bkqvgd).
- Rejected: publishing from the integration branch, or making it the default branch. 0.3.0's provenance and the environment's branch policy name `main`.
- Chosen by the worker of mara-a57pzn on 2026-11-22: the path `.github/workflows/release.yml`.
- Rejected: a new workflow file name. The Trusted Publisher entries of the 0.3.0 packages and their provenance name `release.yml`.
- Chosen by the maintainer on 2026-11-22: both workflows in the form they keep for 0.4.0 and after; `dist_tag` is the maintainer's input.
- Chosen by the maintainer on 2026-11-23: the workflow starts only by a manual dispatch, as nx-rust's; the dry run before a release is one dispatch.
- Rejected by the maintainer: a dry run on each push to `main` and on each pull request that changes a release input. Runs of a ref queue, about 30 minutes each on four runners: ten merges in three hours are five hours of runs, and a dispatched release waits behind them. A break of the release then shows at the dry dispatch, not at the merge.
- Rejected by the maintainer: gates that hold only for a slice, such as `next` only or no version without a prerelease part.
- Chosen by the maintainer on 2026-11-23: the tag, the GitHub release and its notes come from `nx release changelog`, as in zban and nx-rust, with a `release` block in `nx.json` after theirs (mara-n918j4). His words: "I want the same nx generated release notes like on zban and nx release come on why you thing I sent them as examples?"
- Rejected by the maintainer: GitHub's generated release notes, which the rc.1 slice had chosen. Rejected: 0.3.0's `CHANGELOG.md` with git-cliff, which the maintainer's tooling replaces (mara-4dr2p2).
- Chosen by the worker of mara-n918j4 on 2026-11-23: on a re-run, Tag runs `nx release changelog` only for what is missing: without the tag in full, without the tag and push when only the release is missing, and not at all when the release exists; a release whose tag is missing on `origin` fails the job.
- Rejected: running it as it is on a re-run, as zban's release does. `git tag` refuses the existing tag, and with the tag present Nx's notes start at that tag and hold no commit. Rejected: letting Nx update an existing release, which would replace notes edited since.
- Chosen by the worker of mara-n918j4 on 2026-11-23: Nx's defaults for the tag's message and the release's title, both `v<version>`, and for `strictPreid`.
- Rejected: `Mara <version>`, as rc.1's tag and release had it, which needs a setting the references do not have.
- Chosen by the worker: the version set by hand in `Cargo.toml`.
- Rejected: `nx release version`. It would write the version and `Cargo.lock` in the run and commit them, for one string set by hand, and runs `cargo update --workspace` online (nx-rust's `docs/release.mara.md`).
- Rejected: `nx release publish`. Its Rust task publishes crates with `cargo publish`; the packages are npm tarballs that Publish publishes after their install verification.
- Rejected: the version as an input of the dispatch, as nx-rust's release takes it. `mara --version` and the MCP `serverInfo` come from `Cargo.toml` and would disagree with the package.
- Chosen by the worker: write permissions only in the Publish and Tag jobs, which a dry run never starts.
- Rejected: one job that publishes or, in a dry run, runs `npm publish --dry-run`, as zban's. A dry run would hold `id-token: write` and `contents: write`.
- Chosen by the worker: the release restores no cache.
- Rejected: restoring CI's Cargo and Soufflé caches. A published binary would come from another run's build.
- Chosen by the maintainer on 2026-11-23: the release runs no checks, as nx-rust's and zban's; CI runs them on every pull request and push to `main`, on the build images ([ADR-CI-CHECKS](#01M82C8WTKTJW5PRD24K1XRBES)). Each Build job builds Soufflé from source, which the check jobs built before; a dry run took 11 minutes on 2026-11-23, against about 30 with the check jobs.
- Rejected by the maintainer: check jobs of the release on the build images, about 30 minutes before any build. They ran the suite CI had run on the commit.
- Chosen by the worker of mara-mknkec on 2026-11-23: Prepare does not read CI's result for the recorded commit; `main`'s ruleset, which requires CI's checks before a merge and forbids direct pushes, holds it, as nx-rust's release relies on its protection. A publication runs only from `main`.
- Known limit: the ruleset does not require a branch to be up to date before a merge, so CI's push run on `main` is the first run on the merged commit itself; dispatch a publication after it passed.
- Rejected: Prepare reading the commit's check runs (`checks: read`). It needs the names of the required checks in a second place beside the ruleset, and a dispatch right after a merge would fail while the push run of `main` is still running.
- Chosen by the worker of mara-a3aq0n on 2026-11-23: the setup steps that CI and the release share are the composite action `.github/actions/setup`.
- Rejected: a reusable workflow, which is a job of its own and cannot run before the steps of the calling job. Rejected: a copy of the steps in each job, which drift.
- Chosen by the worker of mara-a3aq0n: the registry is read with `curl` from its abbreviated package document.
- Rejected: `npm view`, which exits 1 both for a missing version and for a failed request (npm 12.2.1), to be told apart only by its message.
- Chosen by the worker of mara-a3aq0n: a dry run leaves out `npm publish --dry-run` for a version that npm has by then, with a warning.
- Rejected: running it anyway. npm 11.5.1 and later refuse a dry run of a published version (`lib/commands/publish.js`; npm 12.2.1 only of one without a prerelease part, as its version check leaves prereleases out), so a dry run that meets a version published after Prepare's check would fail.
- Chosen by the worker of mara-a3aq0n: runs of a ref wait in order with `queue: max`.
- Rejected: GitHub's default queue, which cancels a waiting run when a newer one of the ref arrives: a second dispatch would cancel a dispatched publication that waits.
- Chosen by the worker of mara-a3aq0n on 2026-11-23, after review: the registry helper is a command, not a file the steps source, so that it changes no shell option of the step.
- Chosen by the worker of mara-a3aq0n: the dist-tag baseline is an artifact uploaded before the first publication, kept across re-runs.
- Rejected: a job output, which a re-run of Publish replaces with a snapshot taken after the earlier attempt published.
- Chosen by the worker of mara-a3aq0n: Tag reads the release's assets through the REST API and attaches again an asset not in the state `uploaded`. GitHub's upload API "may leave an empty asset with a state of starter" after a 502 (https://docs.github.com/en/rest/releases/assets#upload-a-release-asset).
- Rejected: telling a missing release by the text of `gh release view`, which is not a stable interface.
- Chosen by the worker of mara-23j2h9 on 2026-11-25: the steps call Bash scripts of `.github/scripts`, moved from the workflow, which `mara-npm:test` runs against loopback stubs; no dry run runs Publish, Registry check or Tag.
- Rejected: the shell inline in the workflow, as zban's and nx-rust's releases keep it, where no test can run it. Rejected: rewriting it in Node, which would change the shell that the rc.1 reviews read.
- Chosen by the worker of mara-23j2h9: Publish passes `--provenance` to the publish script, which passes it to `npm publish`.
- Rejected: the flag inside the script. npm refuses it without a CI provider's OIDC token, so no test could publish to the stub.
- Chosen by the worker of mara-23j2h9: the packages come from `releasePackages` in `packages/mara-npm/scripts/packages.mjs`, the native packages of the dispatcher's table and then `anyPlatformPackages`.
- Rejected: a list of packages in the workflow or a file of its own, a second table beside the dispatcher's. Rejected: the matrices of Build and Registry check from the table, which would add each platform's build image to the dispatcher.
- Chosen by the worker of mara-23j2h9: the tests reach GitHub's API as `gh` reaches its host `github.localhost`, at `http://api.github.localhost`, through the stub as their HTTP proxy.
- Rejected: an HTTPS stub for `GH_HOST=127.0.0.1:<port>`, which needs a certificate made at test time and `SSL_CERT_FILE`. Rejected: replacing `gh` with `curl`, which re-implements its upload.

---

<a id="01M82C8WTKTJW5PRD24K1XRBES"></a>

### Run the local checks' own command in CI

| Type | Decision |
| --- | --- |
| ID | ADR-CI-CHECKS |
| Status | Accepted |
| Justifies | [DES-CI-WORKFLOW](distribution.mara.md#01M82C8WTKYXH8S92ZJQ4FX73R) |

- Chosen by the worker of mara-a57pzn on 2026-11-22: each CI job runs `pnpm run checks`.
- Rejected: a copy of the script's `nx` commands in the workflow. The two would drift.
- Chosen by the worker: no Nx cache between CI runs.
- Rejected: saving `.nx/cache` in the CI cache. Its keys hold no Cargo or Rustfmt configuration outside the repository ([ADR-NX-RUST-TASKS](#01M8094G2DV9JZ35C8VRMFPWYV)), and a restored result checks nothing on the pushed commit.
- Chosen by the maintainer on 2026-11-23: the images `ubuntu-22.04` and `ubuntu-22.04-arm`, the release's build images ([ADR-LINUX-BUILD-IMAGE](#01M82C8WTKAM5575JF0MN21ZA4)), so that a green `main` means the checks passed where the binaries are built, and the release runs none ([ADR-DISPATCHED-RELEASE](#01M82C8WTKPTQCXJQT024J3GP5)).
- Rejected by the maintainer: CI on `ubuntu-24.04` and `ubuntu-24.04-arm` with check jobs of the release on the build images. The suite ran twice, and failures of the build images only (kernel 6.8, 1 ms file timestamps, slower runners: mara-efyg6p, mara-4xsamc) showed only at a release.
- Chosen by the worker: the toolchain that `rust-toolchain.toml` pins, and no other.
- Rejected: a job on the oldest supported Rust, as zban's CI runs. The workspace declares no `rust-version`; it pins one toolchain (AGENTS.md).
- Rejected: `ubuntu-latest`, which moves to a new release without a change in the repository.
- Chosen by the worker: the performance test in every run, on shared runners.
- Rejected: leaving it to the nightly job. [VER-SCALING](verification.mara.md#01M7BF6HGRNQ5SJVNP89G1CCBW) runs it on every pull request, and [ADR-SCALING-CPU-TIME](#01M7DTPFQMCA1VN75741RKEDR6) measured it beside other loads.
- Chosen by the worker of mara-1avrq4 on 2026-11-22: Soufflé's CI cache entry is saved by a step of its own after the checks, also when one failed.
- Chosen by the worker of mara-rw6rhp on 2026-11-22: cargo-nextest comes from nextest's prebuilt release archive, pinned by version and SHA-256 in the workflow, as nx-rust's CI installs it. It downloads about 12 MB per job.
- Rejected: `cargo install --locked cargo-nextest`, which compiles it in every run or needs a cache entry of its own.
- Rejected: a third-party action that installs tools, a further dependency of the workflow.
- Rejected: the cache action's save at the job's end, which runs only for a passing job, so that the next run after a failure builds Soufflé again.
- Chosen by the worker: each push run has a concurrency group of its own; the runs of a pull request share one.
- Rejected: one group per ref without cancelling, in which a newer push run cancels an older one still pending.
- Chosen by the worker: third-party actions pinned by major version, as zban's and nx-rust's workflows pin them.
- Rejected: pinning by commit SHA, which neither reference does.
- Chosen by the worker: each job stops after 60 minutes, over three times a run without caches (18 minutes).
- Chosen by the worker of mara-1avrq4: Cargo's CI cache entry names the profile and target-triple directories under `target/` one by one, under a new key prefix, so that it cannot hold Soufflé and no entry of the earlier design is restored.
- Rejected: `target` with the exclusion `!target/souffle-*`. The cache action archives the directory `target` whole, Soufflé included, so that a changed `scripts/build-souffle.sh` could get the old binary back.

---

<a id="01M7BGRHX3NVNZS0NGQ0BKPSH1"></a>

### Deliver agent guidance from the binary, not a skill

| Type | Decision |
| --- | --- |
| ID | ADR-GUIDE-IN-BINARY |
| Status | Accepted |
| Justifies | [REQ-AGENT-GUIDE](agents.mara.md#01M7BGRHWB2MGXEY5G41YP8E0G) |

- Chosen: the guide is compiled into the binary and delivered by `mara prime` and the MCP server instructions, as [zban](https://github.com/convesoft/zban) does.
- Rejected: a standalone skill, as in 0.3.0. It was installed separately and had to be matched to the binary's version by hand.

---

<a id="01M7BH0XEYXZXY3ZD8NNYGS0GM"></a>

### Without a project, advertise only initialization

| Type | Decision |
| --- | --- |
| ID | ADR-INIT-NOTE-WITHOUT-PROJECT |
| Status | Accepted |
| Justifies | [DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8) |

- Chosen: outside a project, `mara prime` and the MCP instructions say how to initialize one, and nothing more.
- Rejected: the full guide everywhere. It fills the context of projects that do not rely on Mara.
- Rejected: no output at all. Nothing would tell a user or agent how to start.

---

<a id="01M7CMW6YBVK2FYK2AQA7XAT6Y"></a>

### Keep the agent guide independent of the project's schema

| Type | Decision |
| --- | --- |
| ID | ADR-STATIC-AGENT-GUIDE |
| Status | Accepted |
| Justifies | [DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8) |

- Chosen: one guide per binary, which sends the agent to schema inspection.
- Rejected: listing the project's types and ID prefixes. MCP instructions are sent once per connection, so the list goes stale after a schema edit, and a large schema exceeds the size limit.

---

<a id="01M7BF6GRR25T2FSC1KYQYW41V"></a>

### Key derived values by content

| Type | Decision |
| --- | --- |
| ID | ADR-CONTENT-KEYED-VALUES |
| Status | Accepted |
| Justifies | [DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY) |

- Chosen: each derived value has its own key, built from the hashes of the inputs it reads.
- Rejected: one fingerprint over the repository. Any unrelated file, `.git` change, failed request or idle period dropped everything ([EVD-PREVIOUS-IMPLEMENTATION-AUDIT](performance.mara.md#01M7BF6H4V7TRV0EZTK37530GE)).
- Rejected: keys from paths or timestamps.

---

<a id="01M7CMW6YZ0S1DPP7T8AD7A3DQ"></a>

### Filter inputs through the project's `.gitignore` files

| Type | Decision |
| --- | --- |
| ID | ADR-GITIGNORE-FILTERS-INPUTS |
| Status | Accepted |
| Justifies | [DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY) |

- Chosen: include patterns select documents and code files, and `.gitignore` files in the project tree exclude them.
- Rejected: configuration patterns only. Every project would list `target/`, `node_modules/` and its build output itself.
- Rejected: Git's global excludes and `.git/info/exclude`. They differ per machine, so hosts would disagree.

---

<a id="01M7BF6GTKYKE4FRPM8A80VXFJ"></a>

### Gate pull requests on work counters and growth ratios

| Type | Decision |
| --- | --- |
| ID | ADR-COUNTERS-GATE-PULL-REQUESTS |
| Status | Accepted |
| Justifies | [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) |

- Chosen: pull requests are gated on exact work counters and growth ratios ([VER-WORK-COUNTERS](verification.mara.md#01M7BF6HGG6ET88HS641Y5676G), [VER-SCALING](verification.mara.md#01M7BF6HGRNQ5SJVNP89G1CCBW)); absolute time is checked only nightly ([VER-TIME-BUDGETS](verification.mara.md#01M7BF6HH06WEJ67P04H22BM66)).
- Rejected: absolute time thresholds on pull requests. Timings on shared machines varied by about ±20% in the audit; counts and ratios did not.

---

<a id="01M7DTPFQMCA1VN75741RKEDR6"></a>

### Time the scaling test in CPU time, with one early look and 60 runs near the limit

| Type | Decision |
| --- | --- |
| ID | ADR-SCALING-CPU-TIME |
| Status | Accepted |
| Justifies | [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) |

- Chosen: the median CPU time of each size. After five rounds, one look: a pair whose growth is more than four standard errors below the limit passes; every other pair gets 60 runs of both sizes and is judged once on the medians of all runs. The maintainer chose this stopping rule. Beside one looping 16-job release build (load 14–31), 20 of 20 runs passed in 31–49 s, with 20–26 of 32 pairs at 60 runs; at load 13–16 without it a run took about 24 s.
- Chosen margin: four standard errors, from the standard deviation of the pair's ten runs pooled over its two sizes, because a false pass can happen only at the look: in a simulation with 15–25% log-normal noise, 2.0× then 2.4× or 2.5× passed at most 0.012 points more often than with 60 fixed runs, and 3.5 errors still exceeded that.
- Rejected: repeated looks until a pair is clear, then the median of up to 60 runs. They select a passing estimate: at 2.0× then 2.4× with 15% noise they passed 2.32% against 0.66% for 60 fixed runs.
- Rejected: failing a pair still unresolved at 60 runs. It fails unchanged code whose growth lies within the noise of 60 runs below the limit.
- Rejected: two standard errors from the median absolute deviation of five runs per size. Five runs underestimate the noise: the look added up to 2.6 points of passes above the limit.
- Rejected: 60 runs of every pair. It took 35 s instead of 24 s at load 13–16, with no measurably fewer false passes.
- Rejected: wall-clock time. It counts waiting for a CPU: under two looping release builds, 11 of 20 runs failed at five rounds against 6 of 20 with CPU time.
- Rejected: the minimum over rounds. A short run escapes contention more often than a long one, so the minimum inflates growth: 7 to 10 of 10 busy runs failed, against 1 to 3 with the median.
- Rejected: pinning the run to one CPU, or disabling transparent huge pages or address randomization for it. None narrowed the spread.
- Decided by the orchestrator on the maintainer's behalf on 2026-11-16: the daemon also has scaling cases, since most requests take that path. Chosen for them: the daemon's process CPU clock, in nanoseconds and with its exited threads. Rejected: `utime` and `stime` of `/proc/<pid>/stat`, in 10 ms ticks, coarser than a warm request.
- Not covered: CPU time excludes time a process waits on I/O or locks, so a blocking latency that grows faster than the corpus does not fail this test.
- Rejected by the maintainer: raising the limit to 2.4 so that the test also passes on a machine whose memory bandwidth other processes saturate. Correct code would pass there by only 0.04–0.09, and the weakest planted fault would fail by 0.15 instead of 0.25; the maintainer chose to rerun such a failure without that load instead.

---

<a id="01M7F8TYKM1HAKA1YQ5NFNKBJC"></a>

### Alternate the hub's other row keys between the lengths of `Type` and `ID`

| Type | Decision |
| --- | --- |
| ID | ADR-HUB-ROW-KEYS |
| Status | Accepted |
| Justifies | [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) |

- Chosen: the hub's other rows alternate between `Tags` and `No`. A comparison of two keys of different lengths ends at the length: with only `Tags` rows, a per-edge scan for the ID cost about 0.4 ns per row and a scan for the type, whose key has the length of `Tags`, about 1.0 ns. The number of rows and the counts stay, and on correct code the hub's CPU time and growth did not change: 166 and 163 ms at 16× quiet, and growth per doubling of 1.95–2.26 under load, against 2.00–2.27 before.
- Measured with each scan planted in a copy, as growth from 4× to 16× before and after; quiet is load below 3 and 9 rounds, the rest the gate's 60 runs per size. The scan for the ID (`value("id")` per edge in `Checker::edge`): 2.46–2.57 and 2.86 quiet, 2.37 and 2.57 beside other projects' tests (load 9–29), 2.46 and 2.61–2.71 beside 32 spinning processes (load 19–48), 2.41 and 2.62–2.65 beside 24 spinning and 8 memory-writing processes (load 34–52). The scan for the type (in `target_type`): 2.98 and 2.81 quiet, 2.67 and 2.55 beside other projects' tests, 2.62 after beside the spinning processes, 2.70 and 2.56 beside the memory-writing ones. Under load the smaller margin of the two scans grew from 0.07–0.16 to 0.25–0.31.
- Rejected: leaving the shape. The scan for the ID exceeded the limit by only 0.06–0.22 under load.
- Rejected: four times as many `Tags` rows. The scan for the ID grew 2.92 at load 10, but the hub took 60% more CPU time per run (198 to 316 ms at 16×).
- Rejected: a block of `Tags` rows, then a block of as many `No` rows. Both scans grew 3.15 quiet, but the hub took 11% more CPU time per run, and correct code failed the gate in 2 of 2 runs beside the memory-writing processes (2.35, 2.36).
- Rejected: only `No` rows. The scan for the ID grew 3.09 quiet, but the scan for the type fell to 2.54.

---

<a id="01M7F9N0JK7BSWE98V9VTCGBV0"></a>

### Exempt the converter from the scaling test

| Type | Decision |
| --- | --- |
| ID | ADR-NO-CONVERTER-SCALING |
| Status | Accepted |
| Justifies | [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |
| Justifies | [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) |

- Chosen by the maintainer on 2026-11-15: `project convert` has no scaling case, and its counter case runs on a small fixed 0.3.0 input. Each user runs the conversion once.
- Rejected: a 0.3.0 mode of the corpus generator. It is test tooling for one operation.
- Rejected: copies of the `v0.3.0` corpus at 1×, 4× and 16× with their MIDs and IDs made unique. It is test tooling for one operation too.

---

<a id="01M8094G2DV9JZ35C8VRMFPWYV"></a>

### Run the repository's tasks with Nx and nx-rust

| Type | Decision |
| --- | --- |
| ID | ADR-NX-RUST-TASKS |
| Status | Accepted |

- Chosen by the maintainer on 2026-11-21: Nx with `@convesoft/nx-rust` runs the repository's tasks. The plugin makes each crate a project with the targets it infers, such as `fmt-check`, `lint`, `check`, `build` and `test`, and adds a project for the workspace. The inferred targets run cargo: Cargo stays the build tool, and a plain cargo command still works.
- Chosen by the maintainer on 2026-11-22: the root files are those of Nx's TypeScript preset (`create-nx-workspace@24.0.6 --preset=ts`), generated outside the checkout, with oxlint and oxfmt in place of its Prettier. Nx and its plugins are pinned at 24.0.6, the version nx-rust is tested with.
- Chosen by the maintainer on 2026-11-22: the plugin comes from the npm registry at an exact version, named in `nx.json` by its package name. The pinned version is `1.0.0-rc.2`, a release candidate (dist-tag `next`). It is younger than pnpm's minimum release age, so `minimumReleaseAgeExclude` in `pnpm-workspace.yaml` exempts exactly that version; remove the entry once the release is older than pnpm's limit, or when the pin moves to `1.0.0`. Every Rust cache key holds the plugin's version.
- Rejected by the maintainer: loading the plugin from the TypeScript source of a local checkout. It runs no `test` and restores no `build`, because two of its helper scripts exist only in a build.
- Rejected by the maintainer: linking to a build of the plugin in a local checkout.
- Rejected by the maintainer: changing the plugin so that it runs from its source.
- Rejected by the maintainer: a vendored tarball.
- Rejected by the maintainer: a GitHub pre-release.
- Rejected: cargo commands and scripts alone. The alpha.6 slice builds JS packages and napi and WASM artifacts beside the crates (mara-xzfeyj), which cargo neither orders nor caches.
- Nx's cache and workspace data stay in the checkout (`cacheDirectory` in `nx.json`), so that worktrees share no result.
- Rejected: Nx 24's default, one per-user folder for every checkout of the repository.
- Cache keys: files outside a crate that `include_str!` reads are its `fileInputs`, which reach every compiling target and the crates that depend on it. What `mara`'s tests read at run time, and no compiling target, is an input of its `test` target: `CI`, `MARA_NO_DAEMON`, the `MARA_TEST_*` hooks, `TMPDIR`, the mode and SHA-256 of the pinned Soufflé in the artifact directory that `CARGO_TARGET_DIR` names, the tree of the tag `v0.3.0`, and for the host tests the browser package's files, its WebAssembly module and Soufflé's, `PLAYWRIGHT_BROWSERS_PATH`, and the output of `node --version` and of `tests/hosts/node/support/browser.mjs`, the module that launches Chromium: Playwright's version, the executable's path, which names its revision, and its size and modification time, or that it is missing. Playwright replaces an executable only by unpacking a new download, which gives it a new modification time.
- Rejected: hashing the executable, some 300 MB, on each key of `mara:test`. `mara:validate` reads `docs/`, `.mara/` and that Soufflé too, and keys them itself (table below).
- Rejected: declaring them for the whole crate (`envInputs`, `runtimeInputs`), which would also run `build`, `check` and `lint` again.
- Chosen by the maintainer on 2026-11-22 (mara-8mxw9c): the tests that check this repository's `docs/` and `.mara/` against the code are a project of their own: the crate `docs-checks`, which is never published (`publish = false`) and on which no package depends. A change to `docs/` or `.mara/` thus runs `docs-checks:test` alone; no test of `mara` or `mara-engine` reads them. A check whose other half tests the product keeps that half in its crate, compared with the template instead of the repository.
- `docs-checks` reads the pages, `.mara/` and `mara`'s templates at run time, and its `test` target keys them with `TMPDIR`, under which its project checks write (chosen during the rewrite on 2026-11-22, mara-8mxw9c).
- Rejected: reading them by `include_str!` with `fileInputs`, which would also run `build`, `check` and `lint` of `docs-checks` again after every page edit.
- Rejected: a hand-written target for these tests, which cannot reuse the plugin's keys.
- Rejected: a configuration of `mara:test`, which shares its inputs.
- Rejected: fixtures copied from the pages, which let a page drift from what the tests check.
- The plugin's keys hold the `clippy.toml` that `CLIPPY_CONF_DIR` selects. They hold no Cargo configuration and no Rustfmt configuration outside the repository: a task that such a file applies to prints a warning that names it when it runs, and a restored task replays the output of the run that stored it. After changing such a Cargo or Rustfmt file, run `nx reset` (nx-rust's `docs/caching.mara.md`, "Cargo configuration that Nx does not hash" and "Rustfmt configuration outside the repository"). `mara:validate` has neither the plugin's keys for these files nor its warnings.
- nx-rust gives an extra target none of the inputs it infers (`docs/configuration.mara.md`, "Changing an inferred target"), so `mara:validate` names the Cargo configuration, the toolchain file and the `CARGO_TARGET_*` variables itself.
- Chosen by the maintainer on 2026-11-22: the local checks run only through Nx, as two `package.json` scripts that combine their targets: `checks` runs every one, and `checks:fast` all but `mara:test:performance`, `mara-engine:build:wasm32` and `mara-wasm:lint:wasm32`. `checks` runs these three configurations as `nx` commands of their own, because `nx run-many` applies a configuration to every project it selects.
- Rejected by the maintainer: keeping the cargo commands in AGENTS.md as a second way to run the local checks. They duplicate what the targets run.
- The scripts pass `--output-style=static`, so that every task prints its log, the test counts included, also when the output is no terminal.
- Rejected: Nx's default output style, which prints no task logs when the output is no terminal.
- CI runs the local checks as `pnpm run checks`, with the steps around it that [DES-CI-WORKFLOW](distribution.mara.md#01M82C8WTKYXH8S92ZJQ4FX73R) gives; the targets that the release runs are named in [DES-RELEASE-WORKFLOW](distribution.mara.md#01M82C8WTK3M2A69XWRGNNQ0SF).
- Chosen by the maintainer on 2026-11-22: every `test` task runs its crate's tests with cargo-nextest and then its doctests with `cargo test --doc` (`testRunner = "nextest"`). A task that names one test target runs only that target. The full suite took 84 s against 159 s with the plugin's default, `cargo test`, results equal (mara-7wysb0).
- Chosen by the worker of mara-rw6rhp on 2026-11-22: the setting is in `[workspace.metadata.nx]` of the root `Cargo.toml`, beside the workspace's other Nx metadata.
- Rejected: the plugin's options in `nx.json`. They apply to every Cargo workspace of the repository, and this one has one.
- nextest counts the `harness = false` binary `diagnostic_memory` of `mara-engine` as one test, which answers nextest's listing (`--list --format terse`); `cargo test` prints no `test result:` line for it. nextest's count is thus cargo's plus one.
- Chosen by the worker of mara-rw6rhp on 2026-11-22: adding `--ignored` to the listing command prints nothing. Other invocations run the allocation check.
- Chosen by the worker of mara-rw6rhp on 2026-11-22: the suite passes with nextest's defaults, so the workspace has no `.config/nextest.toml`.
- With nextest, the key of `test` holds the output of `cargo nextest --version`, a tracked or unignored `.config/nextest.toml` and every `NEXTEST_*` variable. It does not hold an ignored `.config/nextest.toml` or the contents of nextest's user configuration, the file that `NEXTEST_USER_CONFIG_FILE` names or the per-user default; a change to either can restore a result it would change (mara-dt2aez).
- Chosen by the worker of mara-7wysb0 on 2026-11-22: `affectedTests` stays off. Each uncached `test` task, in `checks` and CI, runs its full suite.
- Rejected: `affectedTests`. A change to `mara` or `mara-engine` runs every integration test of `mara`, because those tests can start its binary, the 60 s `cold_counts` among them: 83.6 s against 84.0 s full, and 83.8 s against 83.3 s. A docs-only edit reruns only `docs-checks:test` (about 2 s) when the Rust test results are cached. The coverage baseline does not collect on this repository: 494 of 825 tests fail under instrumentation, and with their causes worked around a limit test and a daemon test still did; one collection took 156 s and 10 GiB (mara-vxeweg).
- The table lists what `checks` runs with the default Cargo configuration. The plugin infers `fmt-check`, `lint` and `test` for each crate and batches their work across crates. The manifests define the extra targets and the configurations `wasm32` and `performance`:

| Target | Runs | Cached | Inputs and dependencies |
| --- | --- | --- | --- |
| `fmt-check` | `cargo fmt --check -p docs-checks -p mara-engine -p mara` | Yes | The plugin's inputs |
| `lint` | `cargo clippy --all-targets --keep-going --message-format=json --workspace --locked`, with warnings denied by `.env.lint` (below), which the plugin applies to cargo's JSON messages | Yes | The plugin's inputs |
| `test` | `cargo nextest run --tool-config-file nx-rust:<a temporary nextest.toml that asks for a JUnit report> --workspace --no-fail-fast --no-tests=pass -E 'package(=docs-checks) \| package(=mara-engine) \| package(=mara)' --locked`, with one `package(=…)` for each crate of the batch; then `cargo test --workspace --doc --no-fail-fast --locked` | Yes | The plugin's inputs, nextest's as above; `mara:test` also the inputs under Cache keys (above) and the dependency on `souffle` (below); `docs-checks:test` also `docs/`, `.mara/`, `crates/mara/templates/` and `TMPDIR` |
| `cargo-workspace:souffle` | `scripts/build-souffle.sh` | No | Output `target/souffle-2.5` |
| `cargo-workspace:souffle-wasm` | `scripts/build-souffle-wasm.sh`, Soufflé's WebAssembly module ([DES-SOUFFLE-BUILD](distribution.mara.md#01M7BF6GX9WNFK70C6H1RBQJ5R)) | No, as `souffle` | Output `target/souffle-2.5-wasm` |
| `mara:validate` | `cargo run --package mara --locked --quiet -- --no-daemon project validate` | Yes | Depends on `mara:build` and `souffle`, whose outputs are in its key; `docs/`, `.mara/`, every file under `crates/` (Rust files, `.gitignore` files and any nested project's `.mara/config.toml`), the root `.gitignore`; the root `.cargo/config.toml` and `.cargo/config`, `rust-toolchain.toml` and every `CARGO_TARGET_*` variable, such as the host's runner, which `cargo run` reads; the mode and SHA-256 of Soufflé; the files of the per-user grammar cache, links followed, in the XDG folder and in macOS's `~/Library/Caches`; `TMPDIR` and the `MARA_TEST_*` hooks that the debug build reads |
| `mara:test:performance` | `cargo nextest run --no-tests=pass -p '<package ID of mara>' --test performance --cargo-profile release --locked`, with no doctest step, as the configuration `performance` of `mara:test`, with `test = "performance"` and `profile = "release"` | Never restored: `checks` runs it with `--skip-nx-cache` | Those of `mara:test`, the dependency on `souffle` included; an `nx` command of its own after the other checks |
| `mara-wasm:build` | `cargo build -p '<package ID of mara-wasm>' --target wasm32-unknown-unknown --release --locked`: the browser package's WebAssembly module ([ADR-RAW-WASM-MODULE](hosts/index.mara.md#01M896GJAKXNE1E6VXT4JYD2WQ)), by the inferred `build`'s default options `targetTriple = "wasm32-unknown-unknown"` and `profile = "release"`, since Nx's `dependsOn` cannot name a configuration | Yes | The plugin's inputs; its target table adds the module, `mara_wasm.wasm`, to the outputs, which the plugin gives a `cdylib` as native library files only |
| `mara-browser:build` | `node packages/mara-browser/copy-module.mjs`: copies the module to `packages/mara-browser/mara.wasm`, and Soufflé's `souffle.js` and `souffle.wasm` beside it, and fails, naming mara-6z7bjs, where `cargo metadata` names a target directory other than `target/`, in which a restored `mara-wasm:build` leaves no module | Yes | Depends on `mara-wasm:build` and `cargo-workspace:souffle-wasm`; the modules and `souffle.js`, through `dependentTasksOutputFiles`; the script, the root `.cargo/config.toml` and `.cargo/config`, `rust-toolchain.toml`, `CARGO_TARGET_DIR` and `CARGO_BUILD_TARGET_DIR` |
| `mara-wasm:lint:wasm32` | The `lint` command with `--target wasm32-unknown-unknown` and without `--all-targets`, as the configuration `wasm32` of the inferred `lint`, with `targetTriple = "wasm32-unknown-unknown"` and `allTargets = false`: `mara-wasm`, the `mara` library and the engine as the module builds them, which the native lint never compiles | Yes | The plugin's inputs; the configuration is in the key |
| `mara-engine:build:wasm32` | `cargo build -p '<package ID of mara-engine>' --target wasm32-unknown-unknown --locked`, as the configuration `wasm32` of the inferred `build`, with `targetTriple = "wasm32-unknown-unknown"` | Yes | The plugin's inputs; the configuration is in the key |
| `cargo-workspace:zban-check` | `zban check` | Yes | `.zban/`, archive included; the `zban` that the command runs, the devDependency in `node_modules/.bin`, which Nx puts first on the command's `PATH`, else the one on `PATH`: its `--version` output, which is the dispatcher's error where its native package is missing, and the SHA-256 of the file it starts from |
| `mara-npm:lint`, `mara-browser:lint` | Oxlint on `packages/mara-npm` and on `packages/mara-browser` | Yes | Inputs inferred by `@nx/oxlint` |
| `mara-npm:test` | `node --test "packages/mara-npm/tests/*.test.mjs"`, the tests of the release workflow's scripts ([DES-RELEASE-WORKFLOW](distribution.mara.md#01M82C8WTK3M2A69XWRGNNQ0SF)) | No: they run the machine's `npm`, `git`, `curl`, `gh`, `jq` and `cargo`, whose versions decide the result | None |

- `mara:test` also depends on `souffle`, and on `mara-browser:build` for the host tests. Nx keys a task with a `dependentTasksOutputFiles` input only after its dependencies have run: the plugin gives `mara:test` one, and `mara:validate` declares one, so both key the Soufflé that `souffle` built in the same run.
- `souffle` is not cached: what it builds depends on the host's C++ toolchain and a download, and with the binary present it only checks it, in about a second. `souffle-wasm` is not cached for the same reasons, the Emscripten SDK on `PATH` taking the C++ toolchain's place; with the module present it checks it in under a second.
- Rejected: caching `souffle` with the script and the tool versions as inputs. A fresh worktree's cache is empty, so a hit could only follow a deleted `target/`.
- `checks` never restores `mara:test:performance`, because a restored timing measures nothing, and selects it in its last `nx` command, where Nx runs `lockfile`, `souffle` and `mara:build` before it, so that no other check runs beside it; other processes on the machine are the caller's matter, and AGENTS.md says how on the maintainer's machine. Run by hand without `--skip-nx-cache`, Nx caches it like any configuration of `test`.
- Rejected: the hand-written target `test-performance` with `cache = false` and `parallelism = false`, which nx-rust 1.0.0-rc.1 required. nx-rust 1.0.0-rc.2's `test` option runs the same test target with `mara:test`'s inputs and dependencies (`docs/testing.mara.md`, "One test target"), and a configuration can set neither `cache` nor `parallelism`.
- Rejected: an extra target with its own command for the `wasm32` build. It would have to repeat the plugin's toolchain and environment inputs.
- Rejected: a root `project.json` for `souffle` and `zban-check`. Nx would merge it into the workspace project, and its name would rename that project.
- Warnings: `.env.lint`, which Nx loads for `lint` tasks only, sets `CARGO_BUILD_WARNINGS=deny`. nx-rust applies it under the `--message-format=json` it passes and keys it (`docs/targets.mara.md`, "Warnings"). A warning in one crate fails every `lint` task of the batch, and a caller's `CARGO_BUILD_WARNINGS` replaces the denial, because Nx does not set a variable over a caller's value.
- Rejected: `CARGO_ENCODED_RUSTFLAGS=-Dwarnings` in `.env.lint`, the workaround for nx-rust 1.0.0-rc.1, which applied no `build.warnings` under JSON messages. It replaced a caller's `RUSTFLAGS` on `lint`.
- Rejected: `RUSTFLAGS` in `.env.lint`, which Nx does not set over a caller's value.
- Rejected: `build.warnings = "deny"` or `-Dwarnings` in `.cargo/config.toml`, which denies warnings for every cargo command.
- Rejected: `[workspace.lints]`, which denies warnings for every cargo command.
- The plugin runs cargo in locked mode (`locked` in `nx.json`), so that no check changes `Cargo.lock`.
- oxfmt leaves out `docs/`, `.mara/` and `.zban/`, whose layouts Mara and zban write; `crates/`, which rustfmt formats and whose test data is compared byte for byte; and the agent files in `.agents/` and `.claude/`.
- The Nx skills live once, in `.agents/skills/`, which Codex and Antigravity read; `.claude/skills` links to it, because Claude Code reads project skills only from there.

