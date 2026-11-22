//! The checks of this repository's `docs/` and `.mara/` against the code (ADR-NX-RUST-TASKS in
//! `docs/decisions.mara.md`): its tests read both at run time from the checkout, so that a change
//! to them reruns these tests alone. The crate is never published and has no code.
