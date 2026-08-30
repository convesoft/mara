# AGENTS.md

Repository-wide instructions for humans and software agents.

## Repository roles

- This repository (`mara`) is the current product and the only implementation
  target.
- `../mara-poc` is a read-only reference. Reuse useful code and knowledge from
  it selectively; do not preserve old decisions merely because they already
  exist there.

## Documentation discipline

- Keep repository-owned documentation, including requirements, definitions,
  architecture, decisions, and specifications, in Git-tracked `*.mara.md`
  files. Tool-required metadata and this instruction file are exceptions, not
  alternative sources of product truth.
- Optimize documentation for retrieval and action: state concrete facts,
  boundaries, invariants, acceptance criteria, and unresolved questions in the
  shortest form that remains unambiguous.
- Prefer precise, testable statements over narrative explanation. Include
  rationale or examples only when they prevent a likely misunderstanding or
  preserve an important decision.
- Do not generate speculative sections, repetitive summaries, generic advice,
  conversational filler, or exhaustive prose “for completeness.” Length is a
  maintenance cost, not a measure of quality.
- Keep each fact in one canonical place. Update or link to existing material
  instead of restating it in multiple documents.
- When changing documentation, preserve useful content but actively remove
  duplication and text that no longer informs implementation, review, or use.

## Delivery discipline

- Prioritize the shortest path to a working end-to-end product. Until the first
  usable alpha exists, optimize for a functioning primary workflow rather than
  completeness, polish, or hypothetical future needs.
- Do not enter open-ended hardening loops. Security, performance, reliability,
  compatibility, error handling, and similar qualities can always be improved;
  stop when the current milestone's explicit requirements are met and no
  critical issue blocks the primary workflow.
- Use test-driven development where practical, but keep the suite proportional
  to the milestone. Cover the primary workflow and at most a few important,
  credible edge cases; do not pursue exhaustive combinations or speculative
  failure modes.
- Prefer safe, observable failure over speculative handling of every possible
  state. Failures must preserve user data and emit actionable, sanitized
  diagnostics. Record unexpected failures in the backlog with reproduction
  context and logs, then add targeted handling and a regression test. Do not
  build speculative recovery or telemetry infrastructure.
- Treat review findings as future work by default. Record non-critical findings
  in the backlog with enough context to revisit them, then return to the current
  milestone instead of expanding its scope.
- Fix a review finding immediately only when it is critical: it prevents the
  primary workflow from working, makes the current release unusable, risks
  unrecoverable user data, or creates a directly exploitable vulnerability in
  the intended deployment. Style, theoretical risk, premature optimization,
  extra defense in depth, and rare non-blocking edge cases are not critical.
- Let later versions improve the product using real usage, feedback, and
  measured constraints. Prefer explicit known limitations in an early working
  release over delaying it for undefined completeness.

## Change discipline

- Do not invent product decisions. When ambiguity materially affects behavior,
  expose the unresolved question instead of silently choosing a plausible
  requirement. Make only small, reversible assumptions when necessary to
  continue.
- Do not claim a feature is complete because mocks, placeholders, or isolated
  tests pass. Completion requires the real primary workflow to run end to end;
  identify any remaining substitute explicitly.
- Do not add abstractions, frameworks, dependencies, tooling, or broad
  refactors for hypothetical reuse or future flexibility. They must solve an
  immediate need of the current milestone.
- Do not invent custom test frameworks, runners, orchestrators, DSLs,
  generators, linters, build systems, or supporting services without explicit
  user approval. Use existing standard tools. If new infrastructure appears
  necessary, stop and present the concrete blocker and smallest alternative.
- Keep changes small and reversible, and preserve a buildable, runnable state
  after each meaningful increment. Avoid broad autonomous rewrites.

## Tooling and versioning

- Implement Mara in Rust. Begin with one Cargo package and split packages or
  crates only when a demonstrated boundary makes the separation useful.
- Commit `Cargo.lock` and pin the Rust toolchain. Keep one project version in
  `[workspace.package].version`; internal packages inherit it rather than
  duplicating version literals.
- Follow Semantic Versioning, beginning at `0.1.0-alpha.0` and progressing
  through `alpha.N`, `beta.N`, and `rc.N` before `0.1.0`. Select versions
  manually during `0.x`; Conventional Commit types do not determine bumps.
- Generate the changelog from Conventional Commit history with `git-cliff`
  when preparing a release; curate the result only when necessary. Do not
  maintain changelog entries manually after every change or add release
  automation before the first release needs it.
- Create immutable annotated `vX.Y.Z[-prerelease]` tags only for releasable
  vertical slices.
- Version persisted document, configuration, index, and protocol formats
  independently when they are introduced; the application version does not
  replace explicit format versions.

## Commit messages

- Use Conventional Commits: `<type>[optional scope][!]: <description>`.
- Use `feat` for user-facing capability, `fix` for a defect, `docs` for
  documentation only, `refactor` for behavior-preserving restructuring,
  `perf` for performance, `test` for tests, `build` for build or dependencies,
  `ci` for CI, and `chore` for otherwise-unclassified maintenance.
- Include a scope only when a clear, stable affected area exists. Reuse names
  established by the repository's modules and recent history; omit the scope
  while the project is too small, and never invent one merely to fill the
  field.
- Write a concise imperative description without a trailing period. Use the
  optional body for motivation or important context and git-trailer-style
  footers for references.
- Mark breaking public-contract changes with `!` and, when useful, a
  `BREAKING CHANGE:` footer describing migration impact.
- Keep each commit to one logical change and stage only intended files.
- After completing and verifying a bounded change, commit it as a logical
  checkpoint unless the user explicitly requests otherwise. Never include
  unrelated changes, amend existing commits, or push without authorization.
