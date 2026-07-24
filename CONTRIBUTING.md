# Contributing to Mara

Mara separates durable engineering meaning from temporary delivery state:

- Git-tracked Mara documents own product goals, requirements, architecture,
  verification intent, and traceability.
- Linear owns delivery priority, sequencing, assignment, and operational state.
- GitHub owns implementation, review, CI, and merge evidence. GitHub Issues are
  the public intake channel, not a second requirements database.

## Before implementation

1. Identify the relevant Mara item IDs in `docs/**/*.mara.md`.
2. Use the assigned Linear issue as the bounded delivery contract when one is
   available. External contributors without Linear access may start from a
   triaged GitHub issue.
3. If product meaning must change, update the authoritative Mara items in the
   same pull request or first obtain the required human decision.
4. Keep the branch and pull request focused on one reviewable outcome. Include
   the Linear identifier, such as `CON-123`, in the branch name when applicable.

Engineering agents must apply the same boundary: read Mara for durable meaning,
read Linear for the current work package, and report implementation evidence in
GitHub.

## Changes and commits

- Use Conventional Commits, for example `feat(parser): extract item blocks` or
  `docs: clarify relation normalization`.
- Preserve user-authored Markdown formatting and keep generated or mechanical
  edits minimal.
- Do not copy requirement bodies or decisions into Linear issues, GitHub issues,
  source comments, or pull requests. Reference their Mara IDs instead.
- Use source-code references only where they provide useful implementation or
  verification traceability.

## Verification

Run the repository checks before requesting review:

```shell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Complete the pull-request template, including the delivery reference, relevant
Mara item IDs, explicit scope, non-goals, and verification evidence.
