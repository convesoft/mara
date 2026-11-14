---
title: Decide platforms and the minimum Node version
status: done
type: task
tags:
  - decision
parent: mara-qzp1gd
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T09:47:39Z
---

Goal: settle the open questions on docs/distribution.mara.md.

- [x] Platforms beyond Linux x64 and arm64 and macOS arm64: none (ADR-KEEP-PLATFORMS-AND-NODE)
- [x] Soufflé explanations on Windows, if Windows is added: Windows is not added
- [x] Minimum Node version: 24
- [x] Whether alpha slices are published to npm under `next`: no, Git tags only (DES-PACKAGES)

## Summary of Changes

The maintainer decided on 2026-11-14: 0.4.0 supports the 0.3.0 platforms (Linux x64 and arm64 with glibc, macOS arm64) and Node 24 (ADR-KEEP-PLATFORMS-AND-NODE); Windows, Intel macOS and musl wait for a request; alpha slices are Git tags only and the first npm release is 0.4.0 (DES-PACKAGES in docs/distribution.mara.md). The untried macOS arm64 Soufflé build stays an open question there and belongs to mara-vt2ktq.

Checks: every item, MID and link in `docs/` checked by script (132 items, 206 links, no problems).
