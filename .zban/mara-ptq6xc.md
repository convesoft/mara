---
title: Run the souffle child process in an operating-system sandbox on Linux
status: todo
type: feature
parent: mara-qzp1gd
blocked_by:
  - mara-bx82k0
discovered_from: mara-t2qrcb
created_at: 2026-11-16T10:42:28Z
---

Goal: a program from an untrusted checkout cannot read or write files outside its private directory or run other code, even when it gets past the allow-list scan or hits a memory fault in Soufflé.

Context: RISK-SOUFFLE-REACHES-OUTSIDE (Accepted; Unlikely, Severe: level HIGH) in docs/rules-facts.mara.md threatens REQ-PROGRAMS-ONLY-COMPUTE (docs/rules.mara.md). The allow-list scan, the private directory and `-l ""` (DES-PROGRAM-PROFILE, DES-RULE-EXECUTION) mitigate it, but five reviews each found a way past earlier, weaker checks (EVD-SOUFFLE-PROGRAM-REACH in docs/rules-evaluation.mara.md), and a memory fault in Soufflé is outside any scan. mara-bx82k0 builds the child-process evaluation that this issue wraps.

Provisional decision, by the orchestrator on 2026-11-16 under the maintainer's rule of that day (it decides on the maintainer's behalf; security matters go ahead on the recommendation, marked provisional, when they can be undone). The question was mara-t2qrcb: an operating-system sandbox, or an accepted Decision that accepts the risk. Chosen: the sandbox, on Linux. Reasons: an Accepted risk of level HIGH has to be treated or accepted, accepting it loosens a security requirement and is the maintainer's alone to choose, and programs come with checkouts the user did not write. The maintainer may overturn this; undo by cancelling this issue and recording a Decision that `Accepts` the risk. It is listed in mara-46j65e.

- [ ] A Design item for the sandbox around the `souffle` child process on Linux (Landlock, no new privileges; decide what the child may read, write and execute, and what happens on a kernel without Landlock), with a Decision item for the choice and the rejected alternative (accepting the risk)
- [ ] The implementation, and tests that a program which reaches outside, by a planted way past the scan, is stopped by the sandbox and reported as a fault (REQ-PROGRAM-FAULTS-REPORTED)
- [ ] The sandbox part of the open question removed from docs/rules-facts.mara.md; RISK-SOUFFLE-REACHES-OUTSIDE names the sandbox as a mitigation
- [ ] macOS (Seatbelt) is not part of this issue: it goes with mara-ag4ees

Confirmed by the maintainer on 2026-11-16, so the decision above is no longer provisional. Chosen option, word for word: "Sandbox on Linux (Recommended) — Keep my decision: a Landlock sandbox around the child process on Linux, built before 0.4.0; macOS follows with the macOS work. Costs one issue of release work, and closes the risk instead of accepting it." Not chosen: accepting the risk; a sandbox after 0.4.0. The question and answer are recorded in mara-46j65e.
