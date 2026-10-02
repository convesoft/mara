# AGENTS.md

Repository-wide instructions for humans and software agents.

## Repository roles

- This repository (`mara`) is the current product and the only implementation
  target.
- `../mara-poc` is a read-only reference. Reuse useful code and knowledge from
  it selectively; do not preserve old decisions merely because they already
  exist there.

## Delivery coordination

- Git-tracked Mara documents own durable product meaning. Work owns delivery
  tracking in Git-tracked `.work/items/<full-id>.md` files: task descriptions,
  expected outcomes, acceptance notes, priority, labels, blockers, and completion.
  No product fact or decision may depend on a Work item. Do not add `task` or
  `release` flavours merely to mirror delivery tracking.
- Use Work through its CLI or MCP tools in the intended Git checkout. Use the
  canonical full item ID in durable references; an unambiguous `w-` prefix is
  sufficient for inspection. Work readiness does not claim or start work.
- GitHub owns pull requests, review discussion, CI, and merge evidence. Treat
  operational views as replaceable projections.
- Release preparation must be owned by one manual Work item. Reuse the item
  that already owns that release; create one only when none exists.
- In a multi-repository workspace, every item must identify its repository and
  cite relevant Mara IDs when they are known. Given an item ID, read the item,
  this file, and its cited Mara items before acting. If the item is unavailable,
  report that instead of inventing its contents.
- When delivery exposes missing, contradictory, or changed product meaning,
  resolve it in the canonical Mara documents; a Work item or review comment
  alone cannot establish the decision.
- Classify findings before creating items. Record accepted current-release work
  as open items with explicit release context, and accepted but unscheduled work
  as open items labeled `backlog`. Create nothing for duplicate, invalid, or
  non-actionable findings. Do not start backlog work unless explicitly requested.
- `backlog this` means create one compact, unscheduled manual Work item labeled
  `backlog` for the referenced actionable outcome. Identify the repository, link
  its source, and include evident Mara IDs; do not change code. If Work is
  unavailable, state that no item was created.
- Keep acceptance checklists and delivery evidence in item bodies. Use Work
  tools for metadata and relations; edit opaque Markdown bodies narrowly,
  preserving their headers, and run Work diagnostics afterward. Use `depends_on`
  for lifecycle prerequisites, not informational links.
- The implementer checks an acceptance box only after producing evidence that
  its criterion passes. Reviewers verify checked criteria and uncheck or request
  evidence for unsupported claims. Close a manual implementation item only after
  all acceptance criteria pass and its change is merged; add criteria during
  review only when the canonical Mara contract changes. Use manual completion
  for acceptance-gated deliverables; a `children` aggregate derives completion
  from its children and is not a separate acceptance approval.

## Pull requests

- Start new item work from current `main` and use `<work-id>-<short-title>`,
  with an unambiguous `w-` prefix for the item, unless the user explicitly selects
  another branch. Do not invent another branch taxonomy or long-lived development
  and release branches.
- Open one pull request per issue against `main`. Its title must follow the
  active commit form and approved-scope list under Commit messages because
  GitHub may use the title for the squash commit.
- Draft every pull request from [`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md).
  Preserve its sections, replace guidance comments with concise task-specific
  facts, and include only issue references, Mara IDs, and verification evidence
  that actually exist; use `N/A` where applicable.
- Squash merge by default. Use rebase merge only when preserving multiple
  independently useful commits that already follow the commit convention.
- When the user intends to publish item work to GitHub or continue its pull
  request, identify the Work item ID, branch, exact ready-to-push SHA, relevant
  Mara IDs, verification evidence, known limitations, and existing PR URL, if
  any. Keep implementation and review decisions in the main thread, and use
  the workflow for the active host:
  - In Pi, the main agent follows the
    [Pi PR flow](.pi/skills/mara-pr-flow-pi/SKILL.md), runs `gh` directly, and uses
    Herdr shell monitoring when explicitly requested. No separate PR agent is
    required. `/pr-status` provides read-only, on-demand inspection.
    `.pi/settings.json` excludes the Codex-only skill from Pi discovery.
  - In Codex, the main thread follows the
    [Mara PR flow](.agents/skills/mara-pr-flow/SKILL.md) and delegates PR operations
    to the project [PR manager](.codex/agents/mara_pr_manager.toml).

## Documentation discipline

- Start at [`docs/index.mara.md`](docs/index.mara.md). Use the
  [Mara skill](skills/mara/SKILL.md) for authoring and the configured
  [schema](.mara/schema.yaml) for flavour selection, fields, and relations.
- Keep repository-owned documentation, including requirements, definitions,
  architecture, decisions, and specifications, in Git-tracked `*.mara.md`
  files. Tool-required metadata and this instruction file are exceptions, not
  alternative sources of product truth.
- Conventional public repository files (`README.md`, `ROADMAP.md`,
  `SECURITY.md`, `LICENSE-*`, and generated `CHANGELOG.md`) may
  remain ordinary Markdown. Keep detailed product and release contracts in
  `*.mara.md`; public files summarize or link to them instead of becoming a
  competing source of truth.
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
- Keep context and explanation as narrative Markdown. Create an item only when
  durable meaning needs its own identity, relations, independent change, or
  verification.
- Keep one independently changeable semantic unit per item. Split content when
  parts can be accepted, rejected, related, implemented, or verified
  separately; keep examples, rationale, and supporting explanation with their
  owning item unless they independently need identity.
- Use only defined flavours, ID prefixes, and relations. If none fits, expose
  the vocabulary gap instead of inventing one silently.
- Add only relations that communicate useful meaning. Never create placeholder
  items or links merely to complete a traceability chain.
- Follow the configured engineering schema and policies. Start new knowledge
  in `draft`; add classifications and coverage as it becomes ready for acceptance.

## Implementation readiness and knowledge maintenance

- Before changing code, read the task, applicable Mara items, and current
  implementation. Determine whether the documented contract is sufficient to
  implement and verify the requested outcome.
- Continue directly when remaining choices are local and reversible. Stop and
  ask one concrete question when missing or contradictory knowledge would
  materially change user-visible behaviour, persisted formats, compatibility,
  data safety, task scope, or acceptance criteria; do not invent the answer.
- Update canonical Mara documents in the same change whenever implementation
  establishes or changes durable product meaning.
- Add or update a `scenario` or `requirement` when observable expected behaviour
  is missing or changed. Add an `actor` only when a durable participant
  distinction affects behaviour or is referenced across multiple items.
- Add or update a `design` when implementation introduces or changes a durable
  interface, persisted format, cross-component contract, architectural boundary,
  or important invariant. Keep routine code organization in code.
- Record a `decision` only for a consequential choice among meaningful
  alternatives: reversing it would change user-visible behaviour, persisted
  formats or compatibility, cross-component architecture, data safety, or
  require meaningful migration. If unresolved, ask before selecting it; once
  settled, record the rationale and relate the decision to the affected
  requirement or design with `justifies`. Do not record routine or easily
  reversible choices.
- Stop when the accepted outcome works end to end, its acceptance criteria have
  evidence, relevant tests pass, and affected durable knowledge is current. Do
  not extend the task into optional hardening, cleanup, or unrelated documentation.

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
- Run project-oriented tests in disposable projects outside the source checkout,
  with test-owned configuration, fixture copies, and Git state. Set child-process
  working directories and isolate inherited Git/configuration state; do not mutate
  the developer's checkout, global configuration, or shared process environment.
  Explicit read-only self-hosting checks may select the real repository.
- Keep fixture documents and synthetic code assertions out of the self-hosted
  corpus and traceability graph. Configure extra language adapters in fixture
  projects only. Real test code remains traceable to the requirements it checks.
  Keep test support proportional, using ordinary Rust tests and `tempfile` with
  small helpers.
- Prefer safe, observable failure over speculative handling of every possible
  state. Failures must preserve user data and emit actionable, sanitized
  diagnostics. Record unexpected failures in the backlog with reproduction
  context and logs, then add targeted handling and a regression test. Do not
  build speculative recovery or telemetry infrastructure.
- Treat a defect as critical only when it prevents the primary workflow, makes
  the current release unusable, risks unrecoverable user data, or creates a
  directly exploitable vulnerability in the intended deployment. Style,
  theoretical risk, premature optimization, extra defense in depth, and rare
  non-blocking edge cases are not critical.
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

## Code review

- Review against the current task, applicable Mara contracts, and the actual
  end-to-end workflow. Report only actionable, consequential defects with a
  concrete impact. Leave deterministic style and formatting checks to CI.
- In ordinary reviews, a finding belongs in the current change only when
  accepted requirements or acceptance criteria are unmet, or when it is critical
  under Delivery discipline. Valid non-critical findings go to the backlog
  without expanding the pull request. Duplicates, resolved findings, speculative
  hardening, theoretical concerns, and non-actionable preferences are dismissed.
- Do not accept mocks, placeholders, or isolated tests as evidence that a real
  primary workflow works. Do not establish new product behavior solely in a
  review comment; update the canonical Mara documents when meaning changes.
- Ordinary reviews cover one bounded batch. Do not enter repeated review-and-fix
  loops after the current contract passes unless a newly found critical issue
  requires action.
- An explicit invocation of the
  [local Codex review loop](.pi/skills/codex-review-loop/SKILL.md) instead opts
  into fixing every finding in the supplied-base diff and repeating review until
  a completed review explicitly reports no findings. The ordinary current-change
  filtering and bounded-batch limits do not apply to this opt-in workflow. If a
  finding cannot be resolved, report the blocker rather than claiming a clean
  review. Product decisions, source safety, verification, and publication
  authorization still follow the other repository rules.

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

- Use Conventional Commits. Mara currently has no approved scopes, so use
  `<type>: <description>` or `<type>!: <description>` for commits and pull
  request titles.
- Use `feat` for user-facing capability, `fix` for a defect, `docs` for
  documentation only, `refactor` for behavior-preserving restructuring,
  `perf` for performance, `test` for tests, `build` for build or dependencies,
  `ci` for CI, and `chore` for otherwise-unclassified maintenance.
- Do not infer a scope from a CLI object (`project`, `schema`, `item`, or
  `relation`), directory name, or existing feature-branch commit. Introduce
  scopes only by updating this section with an explicit approved list after
  stable repository boundaries exist.
- Write a concise imperative description without a trailing period. Use the
  optional body for motivation or important context and git-trailer-style
  footers for references.
- Mark breaking public-contract changes with `!` and, when useful, a
  `BREAKING CHANGE:` footer describing migration impact.
- Keep each commit to one logical change and stage only intended files.
- After completing and verifying a bounded change, commit it as a logical
  checkpoint unless the user explicitly requests otherwise. Never include
  unrelated changes, amend existing commits, or push without authorization.
