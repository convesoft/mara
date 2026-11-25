// .github/scripts/release-tarballs.sh: what a Build job records after its install verification,
// and the check of .github/actions/release-tarballs on every Build job's artifact.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import {
  environment,
  natives,
  others,
  outputs,
  packages,
  run,
  sha256,
  temporaryDirectory,
} from "./support.mjs";

const content = (tarball) => Buffer.from(`the tarball ${tarball}`);

// The table with a second package for any platform, before the dispatcher, as the browser
// package of 0.4.0-rc.2 will be.
const wider = [
  ...natives,
  {
    name: "@convesoft/mara-any-platform",
    native: false,
    tarball: "convesoft-mara-any-platform-0.0.1.tgz",
  },
  ...others,
];
const anyOf = (table) => table.filter((entry) => !entry.native);

// A Build job's dist/npm: the table's packages for any platform and the given native packages.
function distribution(directory, label, nativeEntries, extra = [], table = packages) {
  const dist = path.join(directory, `dist-${label}`);
  fs.mkdirSync(dist, { recursive: true });
  for (const entry of [...anyOf(table), ...nativeEntries]) {
    fs.writeFileSync(path.join(dist, entry.tarball), content(entry.tarball));
  }
  for (const name of extra) {
    fs.writeFileSync(path.join(dist, name), "extra");
  }
  return dist;
}

async function record(directory, dist, label, table = packages) {
  const env = environment(directory, { PACKAGES: JSON.stringify(table) });
  const out = path.join(directory, `out-${label}`);
  const result = await run("release-tarballs.sh", ["record", dist, out, label], {
    cwd: directory,
    env,
  });
  return { result, out };
}

test("record keeps the native tarball and the SHA-256 of every tarball verified", async (t) => {
  const directory = temporaryDirectory(t);
  const { result, out } = await record(directory, distribution(directory, "a", [natives[0]]), "a");
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(fs.readdirSync(out).sort(), [natives[0].tarball, "verified-a.sha256"].sort());
  const lines = fs
    .readFileSync(path.join(out, "verified-a.sha256"), "utf8")
    .trim()
    .split("\n")
    .sort();
  const expected = [...others, natives[0]].map(
    (entry) => `${sha256(content(entry.tarball))}  ${entry.tarball}`,
  );
  assert.deepEqual(lines, expected.sort());
});

for (const [label, nativeEntries, extra] of [
  ["no native tarball", [], []],
  ["two native tarballs", natives.slice(0, 2), []],
  ["a tarball of no package", [natives[0]], ["other-0.0.1.tgz"]],
]) {
  test(`record fails on ${label}`, async (t) => {
    const directory = temporaryDirectory(t);
    const { result, out } = await record(
      directory,
      distribution(directory, "a", nativeEntries, extra),
      "a",
    );
    assert.equal(result.status, 1, result.output);
    assert.match(
      result.stdout,
      /::error::expected the tarballs of the packages for any platform and of one native package/,
    );
    assert.equal(fs.existsSync(out), false);
  });
}

test("record fails without a package for any platform", async (t) => {
  const directory = temporaryDirectory(t);
  const dist = distribution(directory, "a", [natives[0]]);
  fs.rmSync(path.join(dist, others.at(-1).tarball));
  const { result } = await record(directory, dist, "a");
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::expected the tarballs of the packages for any platform/);
});

// Every Build job's artifact, downloaded into one directory as the action's merge does.
async function artifacts(directory, table = packages) {
  const merged = path.join(directory, "npm");
  fs.mkdirSync(merged);
  for (const [index, entry] of natives.entries()) {
    const label = `os-${index}`;
    const dist = distribution(directory, label, [entry], [], table);
    const { result, out } = await record(directory, dist, label, table);
    assert.equal(result.status, 0, result.output);
    fs.cpSync(out, merged, { recursive: true });
  }
  // Pack dispatcher's artifact.
  for (const entry of anyOf(table)) {
    fs.writeFileSync(path.join(merged, entry.tarball), content(entry.tarball));
  }
  return merged;
}

async function collect(directory, merged, table = packages) {
  const env = environment(directory, { PACKAGES: JSON.stringify(table) });
  const result = await run("release-tarballs.sh", ["collect", merged], { cwd: directory, env });
  return { result, env };
}

test("collect checks every Build job's tarballs and writes SHA256SUMS in the order of the packages", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  const { result, env } = await collect(directory, merged);
  assert.equal(result.status, 0, result.output);
  assert.equal(outputs(env).directory, merged);
  assert.deepEqual(
    fs.readdirSync(merged).sort(),
    [...packages.map((entry) => entry.tarball), "SHA256SUMS"].sort(),
  );
  assert.equal(
    fs.readFileSync(path.join(merged, "SHA256SUMS"), "utf8"),
    packages.map((entry) => `${sha256(content(entry.tarball))}  ${entry.tarball}\n`).join(""),
  );
});

test("collect fails without any record", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  for (const name of fs.readdirSync(merged).filter((name) => name.endsWith(".sha256"))) {
    fs.rmSync(path.join(merged, name));
  }
  const { result, env } = await collect(directory, merged);
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::no Build job recorded the tarballs/);
  assert.equal(outputs(env).directory, undefined);
});

test("collect fails when two Build jobs verified different tarballs of one name", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  const record = path.join(merged, "verified-os-1.sha256");
  const lines = fs
    .readFileSync(record, "utf8")
    .replace(
      new RegExp(`^[0-9a-f]{64}(  ${others.at(-1).tarball.replaceAll(".", "\\.")})$`, "m"),
      `${"0".repeat(64)}$1`,
    );
  fs.writeFileSync(record, lines);
  const { result } = await collect(directory, merged);
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    new RegExp(
      `::error::the Build jobs verified different tarballs named ${others.at(-1).tarball}`,
    ),
  );
});

test("collect fails when a Build job's artifact is missing", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  fs.rmSync(path.join(merged, "verified-os-1.sha256"));
  fs.rmSync(path.join(merged, natives[1].tarball));
  const { result } = await collect(directory, merged);
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::expected exactly the tarballs of every package/);
});

test("collect fails on a downloaded tarball that no Build job verified", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  fs.writeFileSync(path.join(merged, "other-0.0.1.tgz"), "other");
  const { result } = await collect(directory, merged);
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::expected exactly the tarballs of every package/);
});

test("collect fails on a tarball that differs from what its Build job verified", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  fs.writeFileSync(path.join(merged, natives[0].tarball), "changed");
  const { result } = await collect(directory, merged);
  assert.notEqual(result.status, 0, result.output);
  assert.match(result.output, new RegExp(`${natives[0].tarball}: FAILED`));
});

test("collect fails on a record line that is not sha256sum's", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  fs.appendFileSync(path.join(merged, "verified-os-0.sha256"), `${natives[0].tarball}\n`);
  const { result } = await collect(directory, merged);
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::verified-os-0\.sha256 has a line that is not sha256sum's/);
});

test("record takes every package for any platform of the table and one native tarball", async (t) => {
  const directory = temporaryDirectory(t);
  const dist = distribution(directory, "a", [natives[0]], [], wider);
  const { result, out } = await record(directory, dist, "a", wider);
  assert.equal(result.status, 0, result.output);
  const recorded = fs.readFileSync(path.join(out, "verified-a.sha256"), "utf8");
  assert.equal(recorded.trim().split("\n").length, 3);
  assert.deepEqual(fs.readdirSync(out).sort(), [natives[0].tarball, "verified-a.sha256"].sort());
});

test("with that table, record still refuses two native tarballs or a missing package", async (t) => {
  const directory = temporaryDirectory(t);
  const two = await record(
    directory,
    distribution(directory, "two", natives.slice(0, 2), [], wider),
    "two",
    wider,
  );
  assert.equal(two.result.status, 1, two.result.output);
  const missing = distribution(directory, "missing", [natives[0]], [], wider);
  fs.rmSync(path.join(missing, wider.at(-2).tarball));
  const absent = await record(directory, missing, "missing", wider);
  assert.equal(absent.result.status, 1, absent.result.output);
  for (const { result } of [two, absent]) {
    assert.match(result.stdout, /::error::expected the tarballs of the packages for any platform/);
  }
});

test("collect takes every Build job's tarballs with that table", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory, wider);
  const { result } = await collect(directory, merged, wider);
  assert.equal(result.status, 0, result.output);
  assert.equal(
    fs.readFileSync(path.join(merged, "SHA256SUMS"), "utf8"),
    wider.map((entry) => `${sha256(content(entry.tarball))}  ${entry.tarball}\n`).join(""),
  );
});

test("collect fails on a file that is neither a tarball nor a record, which Tag would attach", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  fs.writeFileSync(path.join(merged, "notes.txt"), "stray");
  const { result, env } = await collect(directory, merged);
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    /holds notes\.txt, which is neither a tarball nor a Build job's record/,
  );
  assert.equal(outputs(env).directory, undefined);
});

test("record takes a table without packages for any platform, and one native tarball", async (t) => {
  const directory = temporaryDirectory(t);
  const dist = distribution(directory, "n", [natives[0]], [], natives);
  const { result, out } = await record(directory, dist, "n", natives);
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(fs.readdirSync(out).sort(), [natives[0].tarball, "verified-n.sha256"].sort());
});

test("collect fails on a hidden entry, which neither it nor Tag would otherwise see", async (t) => {
  const directory = temporaryDirectory(t);
  const merged = await artifacts(directory);
  fs.writeFileSync(path.join(merged, ".stray"), "hidden");
  const { result, env } = await collect(directory, merged);
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /holds \.stray, which is neither a tarball nor a Build job's record/);
  assert.equal(outputs(env).directory, undefined);
});
