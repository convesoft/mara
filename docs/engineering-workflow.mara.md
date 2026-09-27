# Engineering workflow

Use the [engineering profile](engineering-template.mara.md) for a bounded change.
Plan checks before implementation, and revise the owning knowledge when execution
reveals a gap. Supporting explanation stays alongside the item that owns it.

1. **Establish intent.** State the outcome in a goal and describe concrete flows
   as scenarios. Identify actors only where a participant distinction matters.
   Connect scenarios to their goals and relevant actors. Keep unresolved meaning
   in drafts; delivery tasks belong in the delivery system.
2. **Specify behavior and checks.** Extract independently verifiable requirements,
   classify them and connect their origin. Define repeatable verification methods
   where independent identity is useful. Use `verifies` for requirements/designs,
   `validates` for goals/scenarios, or `checks` from ordinary test code. Inspect
   schema guidance before choosing a flavour or relation.
3. **Define the solution.** Record durable interfaces, formats and boundaries as
   designs satisfying requirements. Preserve consequential choices as decisions
   with `justifies`. Identify material risks, affected knowledge and explicit
   treatment. Keep routine code organization in code. Resolve questions that
   materially change acceptance, compatibility or data safety before implementing.
4. **Implement and assess coverage.** Connect real implementation with `implements`;
   use an independently meaningful artifact with `realizes` where appropriate.
   Keep contracts current as implementation clarifies behavior. Run validation
   and assess selected intent, realization and verification coverage. Accepted
   knowledge and connected test code describe obligations and checks; they do not
   establish that the primary workflow ran.
5. **Verify an identifiable candidate.** Run the required checks against a concrete
   source revision or immutable build. Exercise the real workflow and identify
   remaining substitutes. Record useful execution results as evidence linked to
   the method, with result, actual capture time, tested identity, environment,
   scope and limitations. Inspect failures, inconclusive and conflicting results;
   the existence of a passing record does not settle them.
6. **Review readiness and deploy.** Confirm that the proposed artifact is the
   assessed candidate, required checks support its accepted criteria, relevant
   risks have a treatment, and configuration/recovery prerequisites are usable.
   The delivery process owns approval, deployment identity and run records;
   durable operating contracts remain in Mara. A release needs no new flavour
   or knowledge status.
7. **Verify the deployed outcome.** Perform the planned checks in the intended
   environment against the actual deployed artifact. Record the result and
   environment; staging results alone do not establish production behavior.
   Follow the project's recovery process on failure. Feed material gaps back
   into requirements, designs, verification and risk treatment. Preserve truthful
   provenance when replacing superseded evidence.

For a selected accepted requirement, assess realization with:

```sh
mara trace matrix --id REQ-EXPORT \
  --check-file .mara/engineering-checks.yaml \
  --shape urn:mara:rule:realization
```

For a verification, substitute the actual immutable tested revision:

```sh
mara trace matrix --id VER-EXPORT \
  --check-file .mara/engineering-execution.yaml \
  --shape urn:mara:rule:execution \
  --param 'subject_revision=<tested-revision>'
```

The example IDs identify illustrative selections, not repository items.
Mara treats parameter values as literal text: resolve a Git ref before passing it.
Consume all continuation pages. [Matrices](traceability.mara.md) inspect recorded
relationships and conditions; they do not run tests, deploy software, authenticate
reports, choose release criteria or declare production readiness. Completion
requires the intended outcome and its agreed execution evidence.
