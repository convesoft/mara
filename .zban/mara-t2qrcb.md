---
title: "Decide: sandbox the Soufflé process or accept the containment risk"
status: todo
type: task
tags:
  - maintainer
parent: mara-qzp1gd
created_at: 2026-11-14T11:52:52Z
---

Goal: before the release, the maintainer settles the open question on docs/rules-facts.mara.md.

Context: RISK-SOUFFLE-REACHES-OUTSIDE (Unlikely, Severe: level HIGH in the template's matrix). The allow-list scan, the private directory and `-l ""` (DES-PROGRAM-PROFILE, DES-RULE-EXECUTION) mitigate it, but two independent reviews each found a way past the earlier scan (EVD-SOUFFLE-PROGRAM-REACH), and a memory fault in Soufflé is outside any scan. The maintainer chose on 2026-11-14 to keep an operating-system sandbox out of alpha.3 and decide here.

- [ ] Either a Design for an operating-system sandbox around the `souffle` child process (Landlock on Linux, Seatbelt on macOS) with its issue, or an accepted Decision that `Accepts` the risk
- [ ] Limits on output size and memory of an evaluation, or an explicit decision to do without them: a finite program wrote 8 MB in 15 ms in a review probe
- [ ] The open question removed from docs/rules-facts.mara.md
