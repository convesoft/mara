# Rebuild prompt

Start a new thread in the `feature/mara-upgrade` worktree with this prompt:

```text
Read AGENTS.md and UPGRADE.mara.md, then begin the incremental rebuild of the entire Mara repository. The reset, engineering-template initialization, and tooling preparation are complete.

Start with product intent and scenarios. Rebuild documentation, implementation, tests, and supporting tooling in working increments, using the preserved baseline as a reference.

Use the Mara skill and MCP tools to inspect the engineering schema, author structured knowledge and relationships, validate the corpus, and assess traceability. Pass this worktree’s absolute path explicitly to Mara MCP calls.

For each capability:
- Establish its requirements and necessary design and verification knowledge.
- Review existing code and tests; reuse sound implementation and remove duplication only when justified.
- Add meaningful implements/checks links and genuine execution evidence.
- Keep reusable fixtures separate from the canonical corpus. Materialize test projects in isolated temporary directories with their own configuration and adapters. Keep real test functions traceable to requirements.
- Run the relevant real tests and traceability checks, then commit a verified checkpoint.

Keep documentation concise and test support proportional. Follow the completion criteria in UPGRADE.mara.md.

When a material product decision is unresolved, ask me one plain-text question and wait for my answer. Proceed independently with local, reversible implementation choices.
```
