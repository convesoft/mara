"""Launcher contract tests using disposable Git checkouts and a stub Pi executable."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


LAUNCHER = Path(__file__).with_name("review.py").resolve()


class ReviewLauncherTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="pi-review-test.")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repo = self.root / "checkout"
        self.repo.mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        self.env.update(
            PATH=str(self.bin) + os.pathsep + os.environ["PATH"],
            PI_SESSION_ID="test-caller-session", XDG_CACHE_HOME=str(self.root / "cache"),
            XDG_STATE_HOME=str(self.root / "state"), GIT_CONFIG_NOSYSTEM="1",
            GIT_CONFIG_GLOBAL=os.devnull, STUB_CAPTURE=str(self.root / "launch.json"),
        )
        self.git("init", "-b", "base")
        self.git("config", "user.name", "Review test")
        self.git("config", "user.email", "review@example.invalid")
        (self.repo / "source.txt").write_text("base\n")
        self.git("add", "source.txt")
        self.git("commit", "-m", "base")
        self.common = self.git("rev-parse", "HEAD")
        self.git("switch", "-c", "topic")
        (self.repo / "source.txt").write_text("topic\n")
        self.git("commit", "-am", "topic")
        self.git("switch", "base")
        (self.repo / "other.txt").write_text("base advanced\n")
        self.git("add", "other.txt")
        self.git("commit", "-m", "advance base")
        self.base = self.git("rev-parse", "HEAD")
        self.git("switch", "topic")
        (self.bin / "pi").write_text(f"#!{sys.executable}\n" + '''
import json, os, pathlib, sys
if sys.argv[1:] == ['--help']:
    print('  --fast Start fast mode' if not os.environ.get('STUB_NO_FAST') else 'No fast flag')
    sys.exit(0)
if sys.argv[1:2] == ['--list-models']:
    print('openai gpt-6.1-sol' if not os.environ.get('STUB_NO_MODEL') else 'openai other-model')
    sys.exit(0)
pathlib.Path(os.environ['STUB_CAPTURE']).write_text(json.dumps({
    'argv': sys.argv[1:], 'cwd': os.getcwd(),
    'tmp': os.environ['TMPDIR'], 'target': os.environ['CARGO_TARGET_DIR']}))
if os.environ.get('STUB_CHANGE'):
    pathlib.Path(os.environ['STUB_CHANGE']).write_text('changed by stub')
print('{"type":"agent_settled"}')
print('stub stderr, not a live review', file=sys.stderr)
sys.exit(int(os.environ.get('STUB_EXIT', '0')))
''')
        (self.bin / "pi").chmod(0o755)
        # Filesystem classification is stubbed independently of the host's mounts.
        (self.bin / "findmnt").write_text(f"#!{sys.executable}\nimport os\nprint(os.environ.get('STUB_FS', 'ext4'))\n")
        (self.bin / "findmnt").chmod(0o755)

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.repo, env=self.env, stderr=subprocess.DEVNULL, text=True).strip()

    def run_review(self, sha=None):
        return subprocess.run([sys.executable, str(LAUNCHER), sha or self.base], cwd=self.repo, env=self.env, text=True, capture_output=True)

    def evidence(self):
        return next((self.root / "state/pi/verification/test-caller-session").glob("pi-review.*"))

    def test_launch_from_checkout_uses_bundled_assets_and_immutable_merge_base(self):
        result = self.run_review()
        self.assertEqual(result.returncode, 0, result.stderr)
        launch = json.loads((self.root / "launch.json").read_text())
        args = launch["argv"]
        self.assertEqual(args[:9], ["--mode", "json", "--provider", "openai", "--model", "gpt-6.1-sol", "--thinking", "xhigh", "--fast"])
        self.assertEqual(Path(args[10]), LAUNCHER.parent.parent / "assets/reviewer.md")
        self.assertIn(f"git diff {self.common}", args[11])
        self.assertNotIn("@BASE_SHA@", args[11])
        self.assertNotIn("--approve", args)
        self.assertEqual(launch["cwd"], str(self.repo))
        self.assertEqual(launch["target"], str(self.root / "cache/pi/scratch/test-caller-session/target"))
        run = self.evidence()
        context = json.loads((run / "context.json").read_text())
        self.assertEqual(context["supplied_base_sha"], self.base)
        self.assertEqual(context["review_base_sha"], self.common)
        self.assertEqual(json.loads((run / "status.json").read_text()), {"pi_exit_status": 0, "checkout_unchanged": True})
        self.assertIn("stub stderr", (run / "stderr.log").read_text())
        self.assertIn("REVIEW_DONE", result.stdout)

    def test_failed_pi_is_not_success(self):
        self.env["STUB_EXIT"] = "42"
        result = self.run_review()
        self.assertEqual(result.returncode, 42)
        self.assertEqual(json.loads((self.evidence() / "status.json").read_text())["pi_exit_status"], 42)

    def test_existing_untracked_content_change_invalidates_review(self):
        path = self.repo / "untracked.txt"
        path.write_text("before")
        self.env["STUB_CHANGE"] = str(path)
        result = self.run_review()
        self.assertEqual(result.returncode, 2)
        self.assertFalse(json.loads((self.evidence() / "status.json").read_text())["checkout_unchanged"])

    def test_rejects_ref_missing_session_profile_and_ram_storage(self):
        cases = [("base", {}), (self.base, {"PI_SESSION_ID": ""}),
                 (self.base, {"STUB_NO_FAST": "1"}), (self.base, {"STUB_NO_MODEL": "1"}),
                 (self.base, {"STUB_FS": "tmpfs"})]
        original = self.env.copy()
        for sha, changes in cases:
            with self.subTest(changes=changes, sha=sha):
                self.env = {**original, **changes}
                result = self.run_review(sha)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.root / "launch.json").exists())


if __name__ == "__main__":
    unittest.main()
