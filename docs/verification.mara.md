# Verification

---

<a id="01M7BF6HGG6ET88HS641Y5676G"></a>

### Assert exact work counters

| Type | Verification |
| --- | --- |
| ID | VER-WORK-COUNTERS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ) |
| Verifies | [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |

On every pull request, Rust integration tests run the real CLI and daemon on a generated corpus and assert the work counters ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)) exactly. They also assert the bounds of [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) on built-in checks and program runs.

Cold rows hold in every host and run through the real CLI from 0.4.0-alpha.1:

| Situation | Expectation |
| --- | --- |
| Cold request | For each input it reads, 1 file read and 1 hash; 1 parse per document; 1 snapshot build |
| `.git` and undeclared files | 0 file reads, 0 hashes |
| An operation that reads no code ([DES-CODE-READERS](code.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) and runs no program | 0 subprocesses, 0 code scans |

Warm rows need a host that keeps state between requests ([REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ)) and run against the daemon from 0.4.0-alpha.4:

| Situation | Expectation |
| --- | --- |
| Unchanged warm request, no input modified in the last 2 s | 0 hashes, 0 parses, 0 snapshot builds |
| Unchanged warm request, an input modified in the last 2 s | That input hashed again; 0 parses, 0 snapshot builds |
| One document edited | 1 parse, 1 snapshot build, 0 code scans |
| One code file edited | 1 code scan, 0 document parses |
| Validation or gate with an unchanged evaluation key ([DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)) | 0 `souffle` runs |
| Validation or gate after a change | At most 1 `souffle` run per program and set of supplied inputs |
| Unrelated file, `.git` change, failed request, idle period | No invalidation |

---

<a id="01M7BF6HGRNQ5SJVNP89G1CCBW"></a>

### Measure growth per doubling

| Type | Verification |
| --- | --- |
| ID | VER-SCALING |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |

On every pull request, Rust integration tests time each operation on a release build at each pull-request corpus size (median of runs). They fail when growth per doubling exceeds the limit.

---

<a id="01M7BF6HH06WEJ67P04H22BM66"></a>

### Check time budgets nightly

| Type | Verification |
| --- | --- |
| ID | VER-TIME-BUDGETS |
| Status | Draft |
| Methods | Test |
| Verifies | [REQ-TIME-BUDGETS](performance.mara.md#01M7BF6H47MJ8H16R3B4S7750T) |
| Verifies | [REQ-READS-DURING-VALIDATION](performance.mara.md#01M7BF6H3T3589901T1AE0ADHM) |

Nightly on a reference machine, `hyperfine` times release builds end to end against the budgets in each mode: CLI direct, daemon cold, daemon warm, daemon after a watched edit, and browser-local WASM once it exists. One run issues a read during a long validation of the same project.

---

<a id="01M7D3NAQHP8HX0X0GCZMXD1Q9"></a>

### Compare hosts on the same project

| Type | Verification |
| --- | --- |
| ID | VER-HOST-COMPARISON |
| Status | Accepted |
| Methods | Test |
| Methods | Inspection |
| Verifies | [REQ-SAME-RESULT-ON-EVERY-HOST](engine.mara.md#01M7BF6GXHZNABX9R7NKPNGJNM) |

Rust integration tests run each operation on the same disposable project in CLI direct mode and in each other host that supports it, and require identical JSON results. Each parity case starts when its host exists:

| Host compared with CLI direct mode | From |
| --- | --- |
| Daemon | 0.4.0-alpha.4 |
| Node library, embedded and through the daemon | 0.4.0-alpha.6 |
| Browser, through the daemon and local WASM | 0.4.0-alpha.6 |

Architecture checks run from 0.4.0-alpha.1. They guard [DES-ENGINE-BOUNDARY](engine.mara.md#01M7BF6GY13YX9PD76M0TQG8YK) and do not establish the requirement without the parity cases:

- The local checks build the engine crate for `wasm32`.
- Review of each change to the engine crate finds no filesystem, process, thread, clock or network access in its code or dependencies.

---

<a id="01M7CT60NQK9TAH7JD15HCR2XE"></a>

### Exercise the engineering template's acceptance cases

| Type | Verification |
| --- | --- |
| ID | VER-TEMPLATE-CASES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-TEMPLATE-ACCEPTANCE-RULES](template.mara.md#01M7CT60KVJ75RV9GPP3SEF0WX) |
| Verifies | [REQ-TEMPLATE-GATES-PROVE-READINESS](template-readiness.mara.md#01M7CT60MNF0XPXR9NB6X74F3B) |

Rust integration tests run the real CLI on disposable projects initialized from `engineering`.

| Case | Expectation |
| --- | --- |
| Accepted requirement with only a draft basis, or only a draft verification | Validation error; the same items as drafts are valid |
| Requirement with two accepted verifications, one passing and the other failing or absent from a complete input | `verified` fails |
| Result for another candidate, or for an older revision of the verification or of the requirement | Does not count |
| Risk on a parent requirement, on its need or on that need's goal | Relevant to the child requirement; a sibling requirement is not judged |
| HIGH risk with a mitigation | Blocks until it is reassessed or an accepted decision accepts it |
| Named validation whose result is absent from a complete input, or is not a pass | `release-ready` fails; a validation that is not named does not block |
| Validation of two needs with a pass for one and, for the other, a failing result or none in a complete input | `release-ready` fails |
| A failure and an unavailable fact on the same item | Failed |
| Item whose only missing fact is a risk level, an implementation link while code is unread, or a result absent from an incomplete input | Unavailable, never passed |
| Accepted requirement without an accepted verification | `verified` fails |
| A failing row of each template invariant and gate | Has an explanation |
| `C supersedes B supersedes A` after B retires | Valid |
| Requirement implemented only through a design | Counts when the design is accepted, not when it is draft |
| A view and a gate over the same items | Report the same facts |
