# Repository rebuild

## Ready to start

- Continue on `feature/mara-upgrade` in this worktree. The reset is complete;
  rebuild the product in a new thread, starting with intent and scenarios.
- The engineering schema, enabled knowledge policies, coverage/execution checks,
  and Rust/JavaScript adapters are configured in `.mara/`.
- Agent/build/release tooling, licenses, template sources, and the Mara skill
  remain. Source, tests, and the previous corpus were deliberately removed.
  Cargo and packaging gates resume when their inputs are reintroduced.
- CLI and MCP use the installed `mara 0.3.0-alpha.1` snapshot. Pass this worktree's
  absolute path to MCP calls. Align the binary and installed skill again when
  their contracts change; editing this worktree does not update those copies.

## Reference

The complete working baseline is `0b47b42e31a9b1dedb71b1af5658a067c0a739a9`.
Use it to recover useful knowledge and code selectively. The preceding detailed
capability inventory is at `bbd66a7:UPGRADE.mara.md`; the sibling MARA-72 worktree
is also a reference. The agreed workflow remains available with:

```sh
git show 0b47b42:docs/engineering-workflow.mara.md
git show 0b47b42:docs/index.mara.md
```

The read-only POC example is `../mara-poc/crates/mara-test-support/src/lib.rs`
(`ProjectSandbox`); reuse its isolation principles with small standard helpers.

## Rebuild checklist

1. Establish concise goals/scenarios, then requirements, designs and verification
   where useful. Use the configured vocabulary and preserve surviving identities.
2. Review every baseline capability and record whether it is retained, changed
   or removed. Settle material behavior/compatibility changes with the user.
3. Restore or improve one working CLI/MCP slice at a time. Review its code and
   tests together; remove tests only when their distinct obligation is redundant
   or obsolete. Sound code can stay unchanged; there is no test-count target.
   Follow the test and fixture isolation rules in `AGENTS.md`. Document selection
   does not limit code scanning: materialize synthetic marked source only inside
   disposable fixture projects.
4. Add meaningful `implements` and `checks` links as code is reviewed. Run real
   checks and record evidence for the actual tested revision. Assess the selected
   scope with the configured matrices; an empty selection proves no coverage.
5. Validate the schema/project and run relevant tests before each checkpoint.
   Keep each restored increment buildable. Traceability checks recorded coverage;
   tests and real workflows establish behavior. Commit bounded changes.

Complete when every baseline capability has a reviewed disposition, retained
behavior works end to end, required checks have genuine evidence, and the new
corpus demonstrates the engineering workflow. The empty project's valid state
does not establish product completion. Reconcile the expanded scope with MARA-72
before publishing; publication and release remain separate actions.
