#!/usr/bin/env python3
"""Run one Pi review and retain evidence; the caller owns fixes and cleanup."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile


SKILL = Path(__file__).resolve().parent.parent


def git(*args, cwd=None):
    return subprocess.check_output(["git", *args], cwd=cwd)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def snapshot(checkout):
    untracked = {}
    for name in git("ls-files", "--others", "--exclude-standard", "-z", cwd=checkout).split(b"\0"):
        if not name:
            continue
        path = checkout / os.fsdecode(name)
        if path.is_symlink():
            content = os.fsencode(os.readlink(path))
        else:
            content = path.read_bytes()
        untracked[os.fsdecode(name)] = digest(content)
    return {
        "head": git("rev-parse", "HEAD", cwd=checkout).decode().strip(),
        "status": git("status", "--porcelain=v1", "--untracked-files=all", cwd=checkout).decode(errors="surrogateescape"),
        "diff_sha256": digest(git("diff", "--no-ext-diff", "--binary", "HEAD", cwd=checkout)),
        "untracked_sha256": untracked,
    }


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def storage_root(variable, fallback, category, session):
    root = Path(os.environ.get(variable) or fallback)
    if not root.is_absolute():
        raise ValueError(f"{variable} must be an absolute path")
    path = root / "pi" / category / session
    path.mkdir(parents=True, exist_ok=True)
    filesystem = subprocess.check_output(
        ["findmnt", "--noheadings", "--target", str(path), "--output", "FSTYPE"], text=True
    ).strip()
    if filesystem in {"tmpfs", "ramfs"}:
        raise ValueError(f"{path} is on {filesystem}; select disk-backed {variable}")
    return path.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base_sha", help="full immutable commit SHA of the supplied local base")
    args = parser.parse_args()
    if not re.fullmatch(r"(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})", args.base_sha):
        parser.error("base_sha must be a full commit SHA, not a branch name or abbreviated SHA")
    session = os.environ.get("PI_SESSION_ID", "")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", session):
        parser.error("run from the calling Pi session with its actual PI_SESSION_ID")
    for tool in ("git", "pi", "findmnt"):
        if not shutil.which(tool):
            parser.error(f"required executable is unavailable: {tool}")

    checkout = Path(git("rev-parse", "--show-toplevel").decode().strip()).resolve()
    supplied = git("rev-parse", "--verify", f"{args.base_sha}^{{commit}}", cwd=checkout).decode().strip()
    if supplied != args.base_sha.lower():
        parser.error("base_sha must identify a commit directly, not a tag object")
    baseline = git("merge-base", "HEAD", supplied, cwd=checkout).decode().strip()
    help_text = subprocess.check_output(["pi", "--help"], cwd=checkout, text=True)
    if not re.search(r"^\s+--fast(?:\s|$)", help_text, re.MULTILINE):
        parser.error("Pi's required --fast extension is unavailable; no review was started")
    models = subprocess.check_output(["pi", "--list-models", "gpt-6.1-sol"], cwd=checkout, text=True)
    if not any(line.split()[:2] == ["openai", "gpt-6.1-sol"] for line in models.splitlines()):
        parser.error("the exact openai/gpt-6.1-sol model is unavailable; no review was started")

    instructions = SKILL / "assets/reviewer.md"
    template = SKILL / "assets/reviewer-prompt.md"
    prompt = template.read_text(encoding="utf-8")
    if "@BASE_SHA@" not in prompt:
        raise ValueError("scope template is missing @BASE_SHA@")
    prompt = prompt.replace("@BASE_SHA@", baseline).rstrip()
    prompt += (
        "\n\nReview only: do not modify the checkout or launch further agents/review loops. "
        "Use the inherited TMPDIR and CARGO_TARGET_DIR; the caller owns their cleanup."
    )
    instructions_bytes = instructions.read_bytes()
    scratch = storage_root("XDG_CACHE_HOME", str(Path.home() / ".cache"), "scratch", session)
    evidence = storage_root("XDG_STATE_HOME", str(Path.home() / ".local/state"), "verification", session)
    temporary, target = scratch / "tmp", scratch / "target"
    temporary.mkdir(exist_ok=True)
    target.mkdir(exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix="pi-review.", dir=evidence))
    command = [
        "pi", "--mode", "json", "--provider", "openai", "--model", "gpt-6.1-sol",
        "--thinking", "xhigh", "--fast", "--append-system-prompt", str(instructions), prompt,
    ]
    environment = os.environ.copy()
    environment.update(TMPDIR=str(temporary), CARGO_TARGET_DIR=str(target))
    before = snapshot(checkout)
    save(run / "before.json", before)
    save(run / "context.json", {
        "checkout": str(checkout), "caller_session_id": session,
        "supplied_base_sha": supplied, "review_base_sha": baseline,
        "scratch_owner_session_id": session, "scratch": str(scratch),
        "TMPDIR": str(temporary), "CARGO_TARGET_DIR": str(target),
        "reviewer_prompt_sha256": digest(instructions_bytes),
    })
    (run / "prompt.txt").write_text(prompt + "\n", encoding="utf-8")
    (run / "command.txt").write_text(shlex.join(command) + "\n", encoding="utf-8")
    print(f"Review evidence: {run}", flush=True)
    status = None
    try:
        with (run / "events.jsonl").open("wb") as stdout, (run / "stderr.log").open("wb") as stderr:
            status = subprocess.run(
                command, cwd=checkout, env=environment, stdin=subprocess.DEVNULL,
                stdout=stdout, stderr=stderr,
            ).returncode
    finally:
        after = snapshot(checkout)
        save(run / "after.json", after)
        save(run / "status.json", {"pi_exit_status": status, "checkout_unchanged": before == after})
        print(f"REVIEW_DONE {run} exit={status} checkout_unchanged={before == after}", flush=True)
    if before != after:
        print("Checkout changed during review; do not accept this result as clean evidence.", file=sys.stderr)
        return 2
    # Exit zero is process completion only. The implementing agent reads the verdict.
    return status if status is not None and status >= 0 else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Review launcher failed: {error}", file=sys.stderr)
        sys.exit(2)
    except KeyboardInterrupt:
        print("Review interrupted; inspect retained evidence and process state before retrying.", file=sys.stderr)
        sys.exit(130)
