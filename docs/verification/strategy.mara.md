# Verification strategy

Mara separates a reusable test definition from evidence that a particular test
was executed. The self-hosted corpus records planned verification beside each
capability, while concrete results will be appended under `docs/evidence/` once
an implementation can produce trustworthy provenance.

## Trace and evidence policy

:::req m_01KY7YA2EY7C7AQ1DRV2CJDHDY
:id: REQ-VERIFICATION-TRACE
:title: Every approved requirement shall have planned verification
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: GOAL-TRACEABILITY

The Mara project schema shall report an approved requirement without an incoming
`verifies` relation as a traceability gap. A test may verify several requirements
when it states observable coverage for each one; the relation alone shall not be
treated as proof that the test has run.
:::

:::req m_01KY7YA2EZBXNTGC2062W8YJ5E
:id: REQ-TEST-DEFINITION-SEMANTICS
:title: Test items shall describe reusable verification intent
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-AUDITABLE

A `test` item shall define objective, method, level, fixtures or preconditions,
procedure at an appropriate abstraction, and expected observable result. Its
status shall describe the definition lifecycle and shall not encode the latest
execution outcome.
:::

:::req m_01KY7YA2F0GY4XD5V2P0VCGE33
:id: REQ-EVIDENCE-SEPARATION
:title: Concrete execution results shall be represented as evidence
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-AUDITABLE

An `evidence` item shall record a passed, failed, or inconclusive result for one
or more test definitions through `evidences`, with capture time and either Git
commit or external URI provenance. Updating a test definition shall not rewrite
the history of prior execution results.
:::

:::req m_01KY7YA2F1TK19HZFACJW6BNRS
:id: REQ-EVIDENCE-HISTORY
:title: Evidence corrections shall preserve the prior record
:status: approved
:level: system
:kind: safety
:priority: must
:derives_from: GOAL-AUDITABLE

Evidence is conceptually append-only. If a result is invalid or its provenance
is wrong, a new evidence item shall supersede the old item and explain the
correction. Existing released evidence shall not be silently edited into a
different result.
:::

## Verification layers and self-hosting gate

:::req m_01KY7YA2F2D28P742NFGKGR77D
:id: REQ-VERIFICATION-LAYERS
:title: Verification shall cover syntax, semantics, workflows, and scale
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: GOAL-SCALABLE

The implementation verification plan shall include unit tests for domain rules,
parser fixtures and source-span round trips, schema and graph integration tests,
CLI golden and exit-code tests, fault-injected edit transactions, property tests
for identities and normalization, cross-platform cases, and the documented
performance corpus.
:::

:::req m_01KY7YA2F3PSV7FQJFX827X6QG
:id: REQ-SELF-HOSTING-GATE
:title: Mara shall validate its own specification as an acceptance gate
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-BOOTSTRAP

The first usable milestone shall run the built `mara check` against this
repository's `.mara/project.toml`, schema, and complete `docs/**/*.mara.md`
corpus. Acceptance requires no error diagnostics, deterministic repeated output,
and no unexplained traceability warnings under the checked-in schema.
:::

:::req m_01KY7YA2F48HVTZPGSAYCMEP0Z
:id: REQ-EVIDENCE-REVISION-ANCHOR
:title: Release evidence shall identify the verified revision
:status: approved
:level: system
:kind: safety
:priority: must
:derives_from: GOAL-GIT-CANONICAL

Evidence used for a release or baseline claim shall identify the exact commit
containing the verified semantic content and implementation, or an immutable
external result URI that itself records that commit. Evidence from a dirty or
unknown worktree shall be visibly non-baseline and shall not satisfy a release
claim by default.
:::

## Verification design and rationale

:::design m_01KY7YA2F56W6KB5WTS5P8K29S
:id: DES-VERIFICATION-CORPUS
:title: Layered fixtures with separate evidence records
:status: accepted
:kind: architecture
:satisfies: REQ-VERIFICATION-TRACE
:satisfies: REQ-TEST-DEFINITION-SEMANTICS
:satisfies: REQ-EVIDENCE-SEPARATION
:satisfies: REQ-EVIDENCE-HISTORY
:satisfies: REQ-VERIFICATION-LAYERS
:satisfies: REQ-SELF-HOSTING-GATE
:satisfies: REQ-EVIDENCE-REVISION-ANCHOR

Feature documents own their planned test items. Versioned fixture projects and
golden outputs exercise those definitions; CI and release workflows create or
link immutable result records separately. The Mara repository itself is both the
primary realistic fixture and the acceptance target.
:::

:::decision m_01KY7YA2F624VECFYJANQACR5X
:id: ADR-0011
:title: Separate verification definitions from execution evidence
:status: accepted
:kind: process
:justifies: REQ-TEST-DEFINITION-SEMANTICS
:justifies: REQ-EVIDENCE-SEPARATION
:justifies: REQ-EVIDENCE-HISTORY
:justifies: DES-VERIFICATION-CORPUS

A test remains useful across many commits, platforms, and runs, whereas an
execution result is true only for a particular context. Separate flavours avoid
mutable passed/failed status on specifications and preserve an auditable history.
:::

:::risk m_01KY7YA2F7RZ5W7SYJ5VCRX1HS
:id: RISK-SELF-VALIDATION-BIAS
:title: Self-hosting alone may reproduce Mara's own parser mistakes
:status: open
:severity: high
:likelihood: medium
:affects: REQ-SELF-HOSTING-GATE
:affects: DES-VERIFICATION-CORPUS

A parser can accept its own corpus while violating the written contract.
Independent malformed fixtures, exact expected diagnostics, low-level grammar
tests, property checks, and generic Markdown rendering inspection are required in
addition to a successful self-check.
:::

## Planned verification artifacts

:::artifact m_01KY7YA2F86GYK80DJBXK9X1XH
:id: ART-VERIFICATION-FIXTURES
:title: Mara conformance fixture corpus
:status: proposed
:kind: document
:uri: tests/fixtures
:implements: DES-VERIFICATION-CORPUS
:implements: REQ-VERIFICATION-LAYERS

The fixture corpus will contain minimal valid projects, one-defect projects,
multi-defect projects, source-preservation cases, transaction fault cases, and a
generated scale project with versioned expected results.
:::

:::artifact m_01KY7YA2F9KYJK89B130CAVVMJ
:id: ART-EVIDENCE-CORPUS
:title: Mara verification evidence corpus
:status: proposed
:kind: document
:uri: docs/evidence
:implements: DES-VERIFICATION-CORPUS
:implements: REQ-EVIDENCE-SEPARATION
:implements: REQ-EVIDENCE-REVISION-ANCHOR

The evidence corpus will contain or reference selected baseline and release
results. Routine transient CI output may remain in CI storage when an immutable
URI and revision provide sufficient provenance.
:::

## Planned meta-verification

:::test m_01KY7YA2FACQJ7WHG8GJBSP03D
:id: TEST-META-TRACEABILITY
:title: Requirement-to-test trace policy test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-VERIFICATION-TRACE
:verifies: REQ-TEST-DEFINITION-SEMANTICS

The self-check shall report approved requirements without tests and test items
without verified or validated targets. Schema fixtures shall prove that status
and relation changes update gaps deterministically without claiming execution.
:::

:::test m_01KY7YA2FBPRE9HWFJV971FYTY
:id: TEST-EVIDENCE-MODEL
:title: Evidence provenance and history test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-EVIDENCE-SEPARATION
:verifies: REQ-EVIDENCE-HISTORY
:verifies: REQ-EVIDENCE-REVISION-ANCHOR

Fixtures shall reject evidence without a test target, capture time, result, or
commit/URI; preserve multiple runs of one test; resolve superseding corrections;
and distinguish baseline commits from dirty or unknown working states.
:::

:::test m_01KY7YA2FCQNXNSFWPRN22PBNT
:id: TEST-VERIFICATION-STRATEGY
:title: Layered verification and self-hosting acceptance test
:status: approved
:kind: validation
:method: automated
:level: acceptance
:verifies: REQ-VERIFICATION-LAYERS
:verifies: REQ-SELF-HOSTING-GATE

The acceptance workflow shall run the layered suites, execute Mara against its
own complete corpus twice, compare deterministic outputs, and fail for any error
or unexplained warning. A separate negative fixture shall prove that the gate
detects a known contract violation.
:::
