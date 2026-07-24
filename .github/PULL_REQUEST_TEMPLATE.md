## Summary

<!-- Describe the bounded delivery change and its observable result. -->

## Delivery reference

- Linear issue: <!-- CON-123, or explain why no Linear issue applies -->
- GitHub issue: <!-- Optional public-intake issue -->

## Mara contract

- Mara item IDs: <!-- REQ-..., ADR-..., TEST-..., or "None" -->
- Contract updates: <!-- Link changed *.mara.md files, or state "None" -->

The repository's Mara documents own durable product meaning. This pull request
records implementation and review evidence; it must not become a competing copy
of requirements or architectural decisions.

## Scope

- <!-- Included change -->

## Non-goals

- <!-- Explicitly excluded work -->

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] Relevant Mara requirements, tests, risks, and decisions remain traceable
