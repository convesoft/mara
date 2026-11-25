// release.yml read as text: the Build and Registry check matrices cover every native package of the
// table, and the workflow sets none of the variables that the scripts read for the tests alone.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { platforms, root } from "../scripts/packages.mjs";

const workflow = fs.readFileSync(path.join(root, ".github", "workflows", "release.yml"), "utf8");

// The platform that a GitHub-hosted runner image builds for, by its name.
function platformOf(image) {
  if (image.startsWith("ubuntu-")) {
    return { os: "linux", cpu: image.endsWith("-arm") ? "arm64" : "x64" };
  }
  if (image.startsWith("macos-")) {
    return { os: "darwin", cpu: image.endsWith("-intel") ? "x64" : "arm64" };
  }
  throw new Error(`no platform is known for the runner image ${image}`);
}

// The `os` entries of a job's matrix, from the job's block of the workflow.
function matrix(job) {
  const block = workflow.match(new RegExp(`^  ${job}:\\n((?:    .*\\n|\\n)*)`, "m"));
  assert.ok(block, `release.yml has no job ${job}`);
  const entries = block[1].match(/^ {8}os: \[([^\]]*)\]$/m);
  assert.ok(entries, `the job ${job} has no matrix of os`);
  return entries[1].split(",").map((entry) => entry.trim());
}

for (const job of ["build", "registry-check"]) {
  test(`the matrix of ${job} has a runner for every native package`, () => {
    const covered = matrix(job).map(platformOf);
    for (const platform of platforms) {
      assert.ok(
        covered.some((entry) => entry.os === platform.os && entry.cpu === platform.cpu),
        `${platform.name} (${platform.os} ${platform.cpu}) has no runner in ${job}`,
      );
    }
  });
}

test("the workflow and its actions set none of the scripts' variables for tests", () => {
  const files = [
    path.join(root, ".github", "workflows", "release.yml"),
    path.join(root, ".github", "actions", "release-tarballs", "action.yml"),
    path.join(root, ".github", "actions", "setup", "action.yml"),
  ];
  for (const file of files) {
    const text = fs.readFileSync(file, "utf8");
    for (const name of [
      "NPM_REGISTRY",
      "NPM_REGISTRY_RETRY_SECONDS",
      "RELEASE_POLL_SECONDS",
      "RELEASE_WAIT_SECONDS",
    ]) {
      assert.ok(!text.includes(name), `${path.relative(root, file)} names ${name}`);
    }
  }
});
