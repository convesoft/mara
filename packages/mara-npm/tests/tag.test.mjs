// .github/scripts/release-tag.sh and release-attach.sh, Tag's steps, with the real git, curl and gh
// against a temporary bare Git remote and the GitHub stub. `nx release changelog` is never run: it
// sends commit authors' email addresses to an outside service. A `pnpm` first on PATH stands in
// for it, logs how it was called and makes what Nx would: the tag, pushed, unless `--git-tag=false`,
// and the release.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { environment, git, run, sha256, startGitHub, temporaryDirectory } from "./support.mjs";

const version = "0.4.0-rc.2";
const tag = `v${version}`;

const nx = `#!/bin/sh
{
  echo "args: $*"
  if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then echo "local tag: yes"; else echo "local tag: no"; fi
  echo "user: $(git config user.name) <$(git config user.email)>"
  if IFS= read -r line; then echo "stdin: $line"; else echo "stdin: empty"; fi
} >>"$NX_LOG"
case " $* " in
  *" --git-tag=false "*) ;;
  *) git tag -a "$TAG" -m "$TAG" && git push -q origin "refs/tags/$TAG" || exit 1 ;;
esac
curl -sSf -o /dev/null -X POST -H "Authorization: Bearer $GH_TOKEN" \\
  -d "{\\"tag_name\\":\\"$TAG\\"}" "$GITHUB_API_URL/repos/$GH_REPO/releases"
`;

async function fixture(t) {
  const directory = temporaryDirectory(t);
  const github = await startGitHub(t);
  const bin = path.join(directory, "bin");
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, "pnpm"), nx, { mode: 0o755 });
  const log = path.join(directory, "nx.log");
  const base = environment(directory, {
    ...github.env,
    PATH: `${bin}:${process.env.PATH}`,
    NX_LOG: log,
    VERSION: version,
    TAG: tag,
  });
  const origin = path.join(directory, "origin.git");
  const checkout = path.join(directory, "checkout");
  git(directory, base, "init", "-q", "--bare", origin);
  git(directory, base, "init", "-q", "-b", "main", checkout);
  git(checkout, base, "commit", "-q", "--allow-empty", "-m", "first");
  const other = git(checkout, base, "rev-parse", "HEAD");
  git(checkout, base, "commit", "-q", "--allow-empty", "-m", "recorded");
  git(checkout, base, "remote", "add", "origin", origin);
  git(checkout, base, "push", "-q", "origin", "main");
  const env = { ...base, SHA: git(checkout, base, "rev-parse", "HEAD") };
  return {
    github,
    env,
    checkout,
    other,
    nxCalls: () => (fs.existsSync(log) ? fs.readFileSync(log, "utf8") : ""),
    remoteTag: () =>
      git(checkout, env, "ls-remote", "origin", `refs/tags/${tag}^{}`, `refs/tags/${tag}`),
    // Standard input that a step which forgot to empty it would pass to Nx.
    tag: () => run("release-tag.sh", [], { cwd: checkout, env, input: "y\n" }),
  };
}

const assertNoConnect = (github) =>
  assert.deepEqual(
    github.log.filter((line) => line.startsWith("CONNECT")),
    [],
  );

test("without the tag and the release, Nx tags, pushes the tag alone and creates the release", async (t) => {
  const f = await fixture(t);
  const result = await f.tag();
  assert.equal(result.status, 0, result.output);
  assert.equal(
    f.nxCalls(),
    `args: nx release changelog ${version} --verbose --git-push-args=--no-follow-tags refs/tags/${tag}\n` +
      "local tag: no\n" +
      "user: github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>\n" +
      "stdin: empty\n",
  );
  assert.match(f.remoteTag(), new RegExp(`^${f.env.SHA}\\s+refs/tags/${tag}\\^\\{\\}$`, "m"));
  assert.equal(
    f.github.log.filter((line) => line.startsWith("POST api.github.localhost/repos/o/r/releases"))
      .length,
    1,
  );
  assertNoConnect(f.github);

  // A re-run finds the annotated tag on the recorded commit and the release, and keeps both.
  const again = await f.tag();
  assert.equal(again.status, 0, again.output);
  assert.match(again.stdout, new RegExp(`the tag ${tag} names ${f.env.SHA}`));
  assert.match(again.stdout, /the release of v0\.4\.0-rc\.2 exists; it and its notes are kept/);
  assert.equal(f.nxCalls().split("args:").length, 2);
});

for (const local of [true, false]) {
  test(`with the tag ${local ? "also local" : "on origin alone"} and no release, Nx creates the release alone`, async (t) => {
    const f = await fixture(t);
    git(f.checkout, f.env, "tag", tag);
    git(f.checkout, f.env, "push", "-q", "origin", `refs/tags/${tag}`);
    if (!local) {
      git(f.checkout, f.env, "tag", "-d", tag);
    }
    const result = await f.tag();
    assert.equal(result.status, 0, result.output);
    assert.equal(
      f.nxCalls(),
      `args: nx release changelog ${version} --verbose --git-tag=false --git-push=false\n` +
        "local tag: no\nuser:  <>\nstdin: empty\n",
    );
    assert.equal(result.stdout.includes(`Deleted tag '${tag}'`), local, result.output);
    assert.equal(f.github.assets(tag).length, 0);
    assert.equal(
      f.github.log.filter((line) => line === "POST api.github.localhost/repos/o/r/releases").length,
      1,
    );
    assertNoConnect(f.github);
  });
}

test("a tag on another commit fails before Nx runs", async (t) => {
  const f = await fixture(t);
  git(f.checkout, f.env, "tag", "-a", tag, "-m", tag, f.other);
  git(f.checkout, f.env, "push", "-q", "origin", `refs/tags/${tag}`);
  const result = await f.tag();
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    new RegExp(`::error::the tag ${tag} names ${f.other}, not the recorded commit ${f.env.SHA}`),
  );
  assert.equal(f.nxCalls(), "");
});

test("a release without its tag on origin fails before Nx runs", async (t) => {
  const f = await fixture(t);
  f.github.release(tag);
  const result = await f.tag();
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    /::error::the release of v0\.4\.0-rc\.2 exists, but origin has no tag/,
  );
  assert.equal(f.nxCalls(), "");
});

test("another answer of GitHub than 200 or 404 fails before Nx runs", async (t) => {
  const f = await fixture(t);
  f.github.lookups(500);
  const result = await f.tag();
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::GitHub answered 500 for the release of v0\.4\.0-rc\.2/);
  assert.equal(f.nxCalls(), "");
});

// The tarballs and SHA256SUMS of the run.
function files(directory) {
  const out = path.join(directory, "attach");
  fs.mkdirSync(out);
  for (const name of ["a-0.4.0-rc.2.tgz", "b-0.4.0-rc.2.tgz", "SHA256SUMS"]) {
    fs.writeFileSync(path.join(out, name), `the file ${name}`);
  }
  return out;
}

async function attach(t, assets) {
  const f = await fixture(t);
  const directory = files(path.dirname(f.checkout));
  if (assets !== undefined) {
    f.github.release(tag, { assets });
  }
  const result = await run("release-attach.sh", [directory], { cwd: f.checkout, env: f.env });
  const attached = Object.fromEntries(
    f.github.assets(tag).map((asset) => [asset.name, asset.content.toString()]),
  );
  return { f, result, attached };
}

const every = {
  "a-0.4.0-rc.2.tgz": "the file a-0.4.0-rc.2.tgz",
  "b-0.4.0-rc.2.tgz": "the file b-0.4.0-rc.2.tgz",
  SHA256SUMS: "the file SHA256SUMS",
};
const uploads = (f) =>
  f.github.log.filter((line) => line.startsWith("POST uploads.github.localhost"));

test("attach uploads every file to a release without assets", async (t) => {
  const { f, result, attached } = await attach(t, []);
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(attached, every);
  assert.equal(uploads(f).length, 3);
  assertNoConnect(f.github);
});

test("attach fails without the release", async (t) => {
  const { f, result } = await attach(t, undefined);
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::GitHub answered 404 for the release of v0\.4\.0-rc\.2/);
  assert.equal(uploads(f).length, 0);
});

test("attach deletes an asset in the state starter and uploads it again", async (t) => {
  const { f, result, attached } = await attach(t, [
    { name: "a-0.4.0-rc.2.tgz", content: Buffer.from(""), state: "starter" },
  ]);
  assert.equal(result.status, 0, result.output);
  assert.match(
    result.stdout,
    /the release's a-0\.4\.0-rc\.2\.tgz is in the state starter; deleting it/,
  );
  assert.deepEqual(attached, every);
  assert.equal(f.github.log.filter((line) => line.startsWith("DELETE")).length, 1);
  assert.equal(uploads(f).length, 3);
});

test("attach keeps an uploaded asset with this run's SHA-256, by its digest or its download", async (t) => {
  const { f, result, attached } = await attach(t, [
    { name: "a-0.4.0-rc.2.tgz", content: Buffer.from(every["a-0.4.0-rc.2.tgz"]) },
    { name: "SHA256SUMS", content: Buffer.from(every.SHA256SUMS), digest: null },
  ]);
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(attached, every);
  assert.deepEqual(uploads(f), ["POST uploads.github.localhost/repos/o/r/releases/1/assets"]);
  assert.equal(
    f.github.log.filter((line) =>
      line.startsWith("GET api.github.localhost/repos/o/r/releases/assets/"),
    ).length,
    1,
  );
  assert.match(result.stdout, /the release has this run's SHA256SUMS/);
});

for (const digest of [`sha256:${sha256("another")}`, null]) {
  test(`attach fails on an uploaded asset with another SHA-256 ${digest === null ? "by its download" : "by its digest"}`, async (t) => {
    const { f, result } = await attach(t, [
      { name: "b-0.4.0-rc.2.tgz", content: Buffer.from("another"), digest },
    ]);
    assert.equal(result.status, 1, result.output);
    assert.match(
      result.stdout,
      new RegExp(
        `::error::the release's b-0\\.4\\.0-rc\\.2\\.tgz has sha256:${sha256("another")}, not this run's`,
      ),
    );
    assert.equal(f.github.log.filter((line) => line.startsWith("DELETE")).length, 0);
  });
}
